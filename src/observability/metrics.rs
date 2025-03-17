use std::time::Instant;

use crate::router::AppState;

use super::{common::OpenTelemetryConfig, get_resource};
use axum::{
    debug_middleware,
    extract::{MatchedPath, Request, State},
    middleware::Next,
    response::IntoResponse,
};
use opentelemetry::{KeyValue, global};
use opentelemetry_otlp::MetricExporter;
use opentelemetry_sdk::metrics::SdkMeterProvider;

const REQUEST_DURATION_METRIC_NAME: &str = "http_requests_duration_seconds";

const EXPONENTIAL_SECONDS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

#[derive(Clone)]
pub struct AppMetricsState {
    pub meter: opentelemetry::metrics::Meter,
    pub counter: opentelemetry::metrics::Counter<u64>,
    pub histogram: opentelemetry::metrics::Histogram<f64>,
}

impl Default for AppMetricsState {
    fn default() -> Self {
        let meter = global::meter("axum-graphql");
        let counter = meter
            .u64_counter("http_requests_total")
            .with_description("Total HTTP requests")
            .with_unit("requests")
            .build();
        let histogram = meter
            .f64_histogram(REQUEST_DURATION_METRIC_NAME)
            .with_description("request duration")
            .with_boundaries(EXPONENTIAL_SECONDS.to_vec())
            .build();

        Self {
            meter,
            counter,
            histogram,
        }
    }
}

#[debug_middleware]
pub async fn track(
    State(AppState {
        metrics: AppMetricsState {
            counter, histogram, ..
        },
    }): State<AppState>,
    req: Request,
    next: Next,
) -> impl IntoResponse {
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

    let attributes = [
        KeyValue::new("method", method.to_string()),
        KeyValue::new("path", path),
        KeyValue::new("status", status),
    ];
    counter.add(1, &attributes);
    histogram.record(latency, &attributes);

    response
}

fn init_provider(config: &OpenTelemetryConfig) -> SdkMeterProvider {
    let exporter = MetricExporter::builder()
        .with_tonic()
        .build()
        .expect("Failed to create metric exporter");

    SdkMeterProvider::builder()
        .with_periodic_exporter(exporter)
        .with_resource(get_resource(config))
        .build()
}

#[must_use]
pub fn init_metrics(config: &OpenTelemetryConfig) -> SdkMeterProvider {
    let meter_provider = init_provider(config);

    global::set_meter_provider(meter_provider.clone());

    meter_provider
}
