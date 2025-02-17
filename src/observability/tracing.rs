use std::env;

use opentelemetry::{global, trace::TracerProvider};
use opentelemetry_otlp::{Protocol, WithExportConfig};
use opentelemetry_sdk::{
    trace::{RandomIdGenerator, Sampler, SdkTracerProvider},
    Resource,
};
use tracing::Level;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct OpenTelemetryConfig {
    opentelemetry_agent_host: String,
    opentelemetry_agent_port: String,
    tracing_service_name: String,
}

/// .
///
/// # Panics
///
/// Panics if `OPENTELEMETRY_ENABLED` environment variable exists and is not either `true` or
/// `false`.
pub fn create_tracing_subscriber_from_env() -> Option<SdkTracerProvider> {
    let opentelemetry_enabled: bool = env::var("OPENTELEMETRY_ENABLED")
        .unwrap_or_else(|_| "false".into())
        .parse()
        .expect("`OPENTELEMETRY_ENABLED` env variable should be either `true` or `false`");

    if opentelemetry_enabled {
        let config = get_opentelemetry_config_from_env();
        let tracer_provider = init_tracer(config);
        let tracer = tracer_provider.tracer("axum-graphql");

        tracing_subscriber::registry()
            .with(tracing_subscriber::filter::LevelFilter::from_level(
                Level::INFO,
            ))
            .with(tracing_subscriber::fmt::layer().with_test_writer())
            .with(OpenTelemetryLayer::new(tracer))
            .init();

        tracing::info!("Tracing subscriber created and initialised");
        Some(tracer_provider)
    } else {
        println!("OpenTelemetry is not enabled, set `OPENTELEMETRY_ENABLED` to true, to enable it");
        tracing_subscriber::registry()
            .with(tracing_subscriber::filter::LevelFilter::from_level(
                Level::INFO,
            ))
            .with(tracing_subscriber::fmt::layer().with_test_writer())
            .init();

        tracing::info!("Tracing subscriber created and initialised");
        None
    }
}

fn init_tracer(config: OpenTelemetryConfig) -> SdkTracerProvider {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(format!(
            "{}:{}",
            config.opentelemetry_agent_host, config.opentelemetry_agent_port
        ))
        .with_timeout(std::time::Duration::from_secs(3))
        .with_protocol(Protocol::Grpc)
        .build()
        .expect("should be using a tokio runtime");

    let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_sampler(Sampler::AlwaysOn)
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(
            Resource::builder()
                .with_service_name(config.tracing_service_name)
                .build(),
        )
        .build();
    global::set_tracer_provider(tracer_provider.clone());

    tracer_provider
}

fn get_opentelemetry_config_from_env() -> OpenTelemetryConfig {
    OpenTelemetryConfig {
        opentelemetry_agent_host: env::var("OPENTELEMETRY_AGENT_HOST")
            .unwrap_or_else(|_| "http://localhost".into()),
        opentelemetry_agent_port: env::var("OPENTELEMETRY_AGENT_PORT")
            .unwrap_or_else(|_| "4317".into()),
        tracing_service_name: env::var("TRACING_SERVICE_NAME")
            .unwrap_or_else(|_| "axum-graphql".into()),
    }
}
