use std::time::Instant;

use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::IntoResponse,
};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

const REQUEST_DURATION_METRIC_NAME: &str = "http_requests_duration_seconds";

pub(crate) fn create_prometheus_recorder() -> PrometheusHandle {
    const EXPONENTIAL_SECONDS: &[f64] = &[
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ];

    PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full(REQUEST_DURATION_METRIC_NAME.to_string()),
            EXPONENTIAL_SECONDS,
        )
        .unwrap_or_else(|_| {
            panic!("Could not initialise the bucket for '{REQUEST_DURATION_METRIC_NAME}'",)
        })
        .install_recorder()
        .expect("Could not install the Prometheus recorder, there might already be an instance running.  It should only be started once.")
}

pub(crate) async fn track_metrics(req: Request, next: Next) -> impl IntoResponse {
    let start = Instant::now();
    let path = if let Some(matched_path) = req.extensions().get::<MatchedPath>() {
        matched_path.as_str().to_owned()
    } else {
        req.uri().path().to_owned()
    };
    let method = req.method().clone();

    let response = next.run(req).await;

    let latency = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let labels = [
        ("method", method.to_string()),
        ("path", path),
        ("status", status),
    ];

    metrics::counter!("http_requests_total", &labels).increment(1);
    metrics::histogram!(REQUEST_DURATION_METRIC_NAME, &labels).record(latency);

    response
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, str};

    use axum::{
        body::Body,
        http::{Request, StatusCode},
        Router,
    };
    use float_cmp::approx_eq;
    use http_body_util::BodyExt;
    use metrics_exporter_prometheus::PrometheusHandle;
    use once_cell::sync::Lazy;
    use prometheus_parse::Scrape;
    use sqlx::sqlite::SqlitePoolOptions;
    use tower::ServiceExt;

    use super::create_prometheus_recorder;
    use crate::{
        main_app, metrics_app, observability::tracing::create_tracing_subscriber_from_env,
    };

    async fn get_app() -> Router {
        let database_url = "sqlite://:memory:";
        let app = main_app(database_url).await;

        let db_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(database_url)
            .await
            .unwrap();

        app.with_state(db_pool)
    }

    static TRACING: Lazy<()> = Lazy::new(|| {
        create_tracing_subscriber_from_env();
    });

    static METRICS: Lazy<PrometheusHandle> = Lazy::new(create_prometheus_recorder);

    #[tokio::test]
    async fn metrics_endpoint_listens_on_initialisation() {
        // arrange
        // Avoid re-initialising the tracing subscriber for each test
        let recorder_handle = Lazy::force(&METRICS);
        Lazy::force(&TRACING);
        let metrics_app_instance = metrics_app(recorder_handle.clone());

        // act
        let response = metrics_app_instance
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();

        // assert
        assert_eq!(response.status(), StatusCode::OK);
    }

    /// Internally, `prometheus_parse` uses an [`std::collections::HashMap`] to store labels, which means the order of the
    /// values will not be deterministic and so not suitable for snapshot testing.  This helper
    /// function creates a [`BTreeMap`] from the labels, and will return labels in a deterministic
    /// order.
    fn sorted_prometheus_metric_labels(labels: &prometheus_parse::Labels) -> BTreeMap<&str, &str> {
        labels
            .iter()
            .fold(BTreeMap::<&str, &str>::new(), |mut acc, (key, val)| {
                acc.insert(key, val);
                acc
            })
    }

    #[tokio::test]
    #[should_panic(
        expected = "Could not install the Prometheus recorder, there might already be an instance running.  It should only be started once.: FailedToSetGlobalRecorder(SetRecorderError { .. })"
    )]
    async fn create_prometheus_emits_error_message_if_called_more_than_once() {
        // arrange
        let _ = Lazy::force(&METRICS);

        // act
        create_prometheus_recorder();

        // assert
    }

    #[tokio::test]
    async fn metrics_endpoint_returns_collected_metrics() {
        // arrange
        // Avoid re-initialising the tracing subscriber for each test
        let recorder_handle = Lazy::force(&METRICS);
        Lazy::force(&TRACING);
        Lazy::force(&TRACING);
        std::env::set_var("OPENTELEMETRY_ENABLED", "true");
        let main_app_instance = get_app().await;
        let metrics_app_instance = metrics_app(recorder_handle.clone());

        // act
        let _ = main_app_instance
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let response = metrics_app_instance
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();

        // assert
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = str::from_utf8(&body_bytes).unwrap();
        let lines: Vec<_> = body.lines().map(|val| Ok(val.to_owned())).collect();
        let Scrape { docs, samples } = prometheus_parse::Scrape::parse(lines.into_iter()).unwrap();
        assert!(docs.is_empty());
        assert!(samples.len() > 3);
        assert_eq!(samples.len() % 4, 0);

        let metric = "http_requests_duration_seconds";
        let sample = samples
            .iter()
            .find(|val| val.metric == metric && val.labels.get("path") == Some("/health"))
            .unwrap_or_else(|| panic!("Missing `{metric}` metric"));
        let prometheus_parse::Value::Histogram(histogram) = &sample.value else {
            panic!("Expected histogram, got {:?}", sample.value);
        };
        assert_eq!(histogram.len(), 12);
        assert!(histogram[0].count >= 1.0);
        assert!(approx_eq!(
            f64,
            histogram[0].less_than,
            0.005,
            epsilon = f64::EPSILON,
            ulps = 2
        ));
        let labels = sorted_prometheus_metric_labels(&sample.labels);
        insta::assert_json_snapshot!(labels);

        let metric = "http_requests_duration_seconds_count";
        let sample = samples
            .iter()
            .find(|val| val.metric == metric)
            .unwrap_or_else(|| panic!("Missing `{metric}` metric"));
        let prometheus_parse::Value::Untyped(count) = &sample.value else {
            panic!("Expected time count, got {:?}", sample.value);
        };
        assert!(*count <= 10.0);
        let labels = sorted_prometheus_metric_labels(&sample.labels);
        insta::assert_json_snapshot!(labels);

        let metric = "http_requests_duration_seconds_sum";
        let sample = samples
            .iter()
            .find(|val| val.metric == metric)
            .unwrap_or_else(|| panic!("Missing `{metric}` metric"));
        let prometheus_parse::Value::Untyped(sum) = &sample.value else {
            panic!("Expected time sum, got {:?}", sample.value);
        };
        assert!(*sum <= 0.001);
        let labels = sorted_prometheus_metric_labels(&sample.labels);
        insta::assert_json_snapshot!(labels);

        let metric = "http_requests_total";
        let sample = samples
            .iter()
            .find(|val| val.metric == metric)
            .unwrap_or_else(|| panic!("Missing `{metric}` metric"));
        let prometheus_parse::Value::Counter(total) = &sample.value else {
            panic!("Expected time count, got {:?}", sample.value);
        };
        assert!(*total > 0.0);
        let labels = sorted_prometheus_metric_labels(&sample.labels);
        insta::assert_json_snapshot!(labels);
    }
}
