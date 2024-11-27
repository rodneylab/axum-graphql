use std::time::Instant;

use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::IntoResponse,
};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};

const REQUEST_DURATION_METRIC_NAME: &str = "http_requests_duration_seconds";

/// Creates a global Prometheus recorder.
///
/// # Panics
///
/// Panics if the recorder has already been initialised.
#[must_use]
pub fn create_prometheus_recorder() -> PrometheusHandle {
    const EXPONENTIAL_SECONDS: &[f64] = &[
        0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
    ];

    PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full(REQUEST_DURATION_METRIC_NAME.to_string()),
            EXPONENTIAL_SECONDS,
        )
        .unwrap_or_else(|_| {
            panic!("Bucket values should not be an empty array (initialising the bucket for '{REQUEST_DURATION_METRIC_NAME}')",)
        })
        .install_recorder()
        .expect("Could not install the Prometheus recorder, there might already be an instance running.  It should only be started once.")
}

pub async fn track(req: Request, next: Next) -> impl IntoResponse {
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
