use std::env;

use opentelemetry::{global, trace::TracerProvider, KeyValue};
use opentelemetry_otlp::{ExportConfig, Protocol, WithExportConfig};
use opentelemetry_sdk::{
    runtime,
    trace::{RandomIdGenerator, Sampler, Tracer},
    Resource,
};
use opentelemetry_semantic_conventions::resource::SERVICE_NAME;
use tracing::Level;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct OpenTelemetryConfig {
    opentelemetry_agent_host: String,
    opentelemetry_agent_port: String,
    tracing_service_name: String,
}

pub fn create_tracing_subscriber_from_env() {
    let opentelemetry_enabled: bool = env::var("OPENTELEMETRY_ENABLED")
        .unwrap_or_else(|_| "false".into())
        .parse()
        .unwrap();

    if opentelemetry_enabled {
        let config = get_opentelemetry_config_from_env();
        let tracer = init_tracer(config);

        tracing_subscriber::registry()
            .with(tracing_subscriber::filter::LevelFilter::from_level(
                Level::INFO,
            ))
            .with(tracing_subscriber::fmt::layer())
            .with(OpenTelemetryLayer::new(tracer))
            .init();

        tracing::info!("Tracing subscriber created and initialised");
    } else {
        println!("OpenTelemetry is not enabled, set `OPENTELEMETRY_ENABLED` to true, to enable");
    }
}

fn init_tracer(config: OpenTelemetryConfig) -> Tracer {
    let tracer_provider = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_export_config(ExportConfig {
                    endpoint: format!(
                        "{}:{}",
                        config.opentelemetry_agent_host, config.opentelemetry_agent_port
                    ),
                    timeout: std::time::Duration::from_secs(3),
                    protocol: Protocol::Grpc,
                }),
        )
        .with_trace_config(
            opentelemetry_sdk::trace::Config::default()
                .with_sampler(Sampler::AlwaysOn)
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(Resource::new(vec![KeyValue::new(
                    SERVICE_NAME,
                    config.tracing_service_name,
                )])),
        )
        .install_batch(runtime::Tokio)
        .expect("should be using a tokio runtime");

    global::set_tracer_provider(tracer_provider.clone());
    tracer_provider.tracer("axum-graphql")
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
