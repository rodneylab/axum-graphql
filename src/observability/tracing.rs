use opentelemetry::{global, trace::TracerProvider};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    logs::SdkLoggerProvider,
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider},
};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

use super::{common::OpenTelemetryConfig, create_format_filter, create_otel_filter, get_resource};

fn init_provider(config: &OpenTelemetryConfig) -> SdkTracerProvider {
    let exporter = SpanExporter::builder()
        .with_tonic()
        .with_endpoint(format!(
            "{}:{}",
            config.opentelemetry_agent_host, config.opentelemetry_agent_port
        ))
        .with_timeout(std::time::Duration::from_secs(3))
        .build()
        .expect("should be using a tokio runtime");

    SdkTracerProvider::builder()
        .with_sampler(Sampler::AlwaysOn)
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(get_resource(config))
        .with_batch_exporter(exporter)
        .build()
}

#[must_use]
pub fn init_tracing(
    config: &OpenTelemetryConfig,
    logger_provider: &SdkLoggerProvider,
) -> SdkTracerProvider {
    let otel_layer = OpenTelemetryTracingBridge::new(logger_provider);
    let otel_layer = otel_layer.with_filter(create_otel_filter());
    let format_layer = tracing_subscriber::fmt::layer()
        .with_thread_names(true)
        .with_filter(create_format_filter());
    let tracer_provider = init_provider(config);
    let tracer = tracer_provider.tracer(config.service_name.clone());
    tracing_subscriber::registry()
        .with(otel_layer)
        .with(format_layer)
        .with(tracing_subscriber::fmt::layer().with_test_writer())
        .with(OpenTelemetryLayer::new(tracer))
        .init();

    global::set_tracer_provider(tracer_provider.clone());
    tracing::info!("Tracing subscriber created and initialised");

    tracer_provider
}
