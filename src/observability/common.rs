use std::{env, sync::OnceLock};

use opentelemetry::KeyValue;
use opentelemetry_sdk::{
    Resource, error::OTelSdkError, logs::SdkLoggerProvider, metrics::SdkMeterProvider,
    trace::SdkTracerProvider,
};
use tracing::Level;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use ulid::Ulid;

use super::{logging::init_logs, metrics::init_metrics, tracing::init_tracing};

pub struct OpenTelemetryConfig {
    pub opentelemetry_agent_host: String,
    pub opentelemetry_agent_port: String,
    pub service_name: String,
}

#[derive(Clone)]
pub struct OpenTelemetryProviders {
    pub logger_provider: SdkLoggerProvider,
    pub tracer_provider: SdkTracerProvider,
    pub meter_provider: SdkMeterProvider,
}

impl Default for OpenTelemetryProviders {
    fn default() -> Self {
        let otel_config = get_opentelemetry_config_from_env();
        let logger_provider = init_logs(&otel_config);
        let tracer_provider = init_tracing(&otel_config, &logger_provider);
        let meter_provider = init_metrics(&otel_config);

        Self {
            logger_provider,
            tracer_provider,
            meter_provider,
        }
    }
}

impl OpenTelemetryProviders {
    #[must_use]
    pub fn new() -> OpenTelemetryProviders {
        Self::default()
    }
}

/// Initialise opentelemetry if the `OPENTELEMETRY_ENABLED` env variable is set to true, otherwise
/// initialise basic terminal logging.
///
/// # Panics
///
/// Panics if `OPENTELEMETRY_ENABLED` environment variable exists and is not either `true` or
/// `false`.
pub fn initialise_observability() -> Option<OpenTelemetryProviders> {
    let opentelemetry_enabled: bool = env::var("OPENTELEMETRY_ENABLED")
        .unwrap_or_else(|_| "false".into())
        .parse()
        .expect("`OPENTELEMETRY_ENABLED` env variable should be either `true` or `false`");

    if opentelemetry_enabled {
        Some(OpenTelemetryProviders::new())
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

#[must_use]
pub fn get_opentelemetry_config_from_env() -> OpenTelemetryConfig {
    OpenTelemetryConfig {
        opentelemetry_agent_host: env::var("OPENTELEMETRY_AGENT_HOST")
            .unwrap_or_else(|_| "http://localhost".into()),
        opentelemetry_agent_port: env::var("OPENTELEMETRY_AGENT_PORT")
            .unwrap_or_else(|_| "4317".into()),
        service_name: env::var("OPENTELEMETRY_SERVICE_NAME")
            .unwrap_or_else(|_| env!("CARGO_CRATE_NAME").into()),
    }
}

pub fn get_resource(config: &OpenTelemetryConfig) -> Resource {
    static RESOURCE: OnceLock<Resource> = OnceLock::new();
    let instance_id = Ulid::new().to_string();

    RESOURCE
        .get_or_init(|| {
            Resource::builder()
                .with_service_name(config.service_name.clone())
                .with_attribute(KeyValue::new("service.instance.id", instance_id))
                .build()
        })
        .clone()
}

/// Build OpenTelemetry filter.
///
/// # Panics
///
/// Panics if unable to parse directives.
#[must_use]
pub fn create_otel_filter() -> EnvFilter {
    EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            format!(
                "{}=debug,tower_http,axum__rejection=trace",
                env!("CARGO_CRATE_NAME")
            )
            .into()
        })
        .add_directive("hyper=off".parse().expect("Should be valid directive"))
        .add_directive(
            "opentelemetry=off"
                .parse()
                .expect("Should be valid directive"),
        )
        .add_directive("tonic=off".parse().expect("Should be valid directive"))
        .add_directive("h2=off".parse().expect("Should be valid directive"))
        .add_directive("reqwest=off".parse().expect("Should be valid directive"))
}

/// Build `EnvFilter` filter.
///
/// # Panics
///
/// Panics if unable to parse directives.
#[must_use]
pub fn create_format_filter() -> EnvFilter {
    EnvFilter::new("info").add_directive(
        "opentelemetry=debug"
            .parse()
            .expect("Should be valid directive"),
    )
}

pub fn shutdown_opentelemetry_providers(opentelemetry_providers: &OpenTelemetryProviders) {
    let OpenTelemetryProviders {
        tracer_provider,
        logger_provider,
        meter_provider,
    } = opentelemetry_providers;
    match tracer_provider.shutdown() {
        Ok(()) => tracing::info!("Tracer provider shutdown."),
        Err(error) => match error {
            OTelSdkError::AlreadyShutdown => tracing::error!("Tracing provider already shut down."),
            OTelSdkError::Timeout(time) => {
                tracing::error!("Tracing provider shutdown timed out after {time:?}");
            }
            OTelSdkError::InternalFailure(error) => {
                tracing::error!("Tracing provider shutdown failed: {error:?}");
            }
        },
    }
    match meter_provider.shutdown() {
        Ok(()) => tracing::info!("Meter provider shutdown."),
        Err(error) => match error {
            OTelSdkError::AlreadyShutdown => tracing::error!("Meter provider already shut down."),
            OTelSdkError::Timeout(time) => {
                tracing::error!("Meter provider shutdown timed out after {time:?}");
            }
            OTelSdkError::InternalFailure(error) => {
                tracing::error!("Meter provider shutdown failed: {error:?}");
            }
        },
    }
    match logger_provider.shutdown() {
        Ok(()) => tracing::info!("Logger provider shutdown."),
        Err(error) => match error {
            OTelSdkError::AlreadyShutdown => tracing::error!("Logger provider already shut down."),
            OTelSdkError::Timeout(time) => {
                tracing::error!("Logger provider shutdown timed out after {time:?}");
            }
            OTelSdkError::InternalFailure(error) => {
                tracing::error!("Logger provider shutdown failed: {error:?}");
            }
        },
    }
}
