use std::{collections::BTreeMap, str};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use float_cmp::approx_eq;
use http_body_util::BodyExt;
use prometheus_parse::Scrape;
use tower::ServiceExt;

use crate::helpers::TestApp;
use axum_graphql::{
    observability::metrics::create_prometheus_recorder, startup::ApplicationRouters,
};

#[tokio::test]
async fn metrics_endpoint_listens_on_initialisation() {
    // arrange
    let ApplicationRouters { metrics_router, .. } = TestApp::spawn_routers().await;

    // act
    let response = metrics_router
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
    let _ = TestApp::spawn_routers().await;

    // act
    let _ = create_prometheus_recorder();

    // assert
}

#[tokio::test]
async fn metrics_endpoint_returns_collected_metrics() {
    // arrange
    let ApplicationRouters {
        main_router,
        metrics_router,
    } = TestApp::spawn_routers().await;

    // act
    let _ = main_router
        .oneshot(Request::get("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let response = metrics_router
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
        .find(|val| val.metric == metric && val.labels.get("path") == Some("/health"))
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
        .find(|val| val.metric == metric && val.labels.get("path") == Some("/health"))
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
        .find(|val| val.metric == metric && val.labels.get("path") == Some("/health"))
        .unwrap_or_else(|| panic!("Missing `{metric}` metric"));
    let prometheus_parse::Value::Counter(total) = &sample.value else {
        panic!("Expected time count, got {:?}", sample.value);
    };
    assert!(*total > 0.0);
    let labels = sorted_prometheus_metric_labels(&sample.labels);
    insta::assert_json_snapshot!(labels);
}
