use opentelemetry_otlp::LogExporter;
use opentelemetry_sdk::logs::SdkLoggerProvider;

use super::common::{OpenTelemetryConfig, get_resource};

/// Create OpenTelemetry logging provider
///
/// # Panics
///
/// Panics if tokio runtime is not available.
#[must_use]
pub fn init_logs(config: &OpenTelemetryConfig) -> SdkLoggerProvider {
    let exporter = LogExporter::builder()
        .with_tonic()
        .build()
        .expect("should be using a tokio runtime");

    SdkLoggerProvider::builder()
        .with_resource(get_resource(config))
        .with_batch_exporter(exporter)
        .build()
}
