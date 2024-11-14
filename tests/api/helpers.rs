use metrics_exporter_prometheus::PrometheusHandle;
use once_cell::sync::Lazy;

use axum_graphql::observability::metrics::create_prometheus_recorder;

pub static METRICS: Lazy<PrometheusHandle> = Lazy::new(create_prometheus_recorder);
