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
        .expect("Could not install the Prometheus recorder")
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
    use std::str;

    use axum::{
        body::Body,
        http::{Request, StatusCode},
        Router,
    };
    use http_body_util::BodyExt;
    use metrics_exporter_prometheus::PrometheusHandle;
    use once_cell::sync::Lazy;
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
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"");
    }

    #[tokio::test]
    async fn metrics_endpoint_returns_collected_metrics() {
        // arrange
        // Avoid re-initialising the tracing subscriber for each test
        let recorder_handle = Lazy::force(&METRICS);
        Lazy::force(&TRACING);
        //let app = get_app().await;
        Lazy::force(&TRACING);
        std::env::set_var("OPENTELEMETRY_ENABLED", "true");
        let main_app_instance = get_app().await;
        let metrics_app_instance = metrics_app(recorder_handle.clone());

        // act
        let _ = main_app_instance
            .oneshot(Request::get("/health_check").body(Body::empty()).unwrap())
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
        let mut lines = body.lines();
        assert_eq!(lines.next(), Some("# TYPE http_requests_total counter"));

        let line_2 = lines.next().unwrap();
        assert_eq!(line_2.find("http_requests_total"), Some(0));
    }
}
