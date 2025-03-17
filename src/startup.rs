use axum::{Router, serve::Serve};
use opentelemetry_sdk::error::OTelSdkError;
use sqlx::SqlitePool;
use tokio::{net::TcpListener, signal};

use crate::{
    database::run_migrations,
    model::get_schema,
    observability::{OpenTelemetryProviders, shutdown_opentelemetry_providers},
    router::init_router,
};

pub struct ApplicationRouter {
    pub router: Router,
}

impl ApplicationRouter {
    /// Build the app router.  This router can be used in unit tests.
    ///
    /// # Errors
    /// Returns an error if the database is not reachable
    pub async fn build(database_url: &str) -> Result<Self, std::io::Error> {
        Ok(Self {
            router: router(database_url).await,
        })
    }
}

/// Listen for and handle shutdown signals.
///
/// # Panics
///
/// Panics if unable to install Ctrl-C handler.
pub async fn shutdown_signal(open_telemetry_providers: Option<OpenTelemetryProviders>) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            tracing::info!("Ctrl-C registered");
            if let Some(value) = open_telemetry_providers {
                shutdown_opentelemetry_providers(&value);
            }
        },
        () = terminate => {
            tracing::info!("Terminate registered");
            if let Some(value) = open_telemetry_providers {
                shutdown_opentelemetry_providers(&value);
            }
        }
    }
}

pub struct Application {
    pub server: Serve<TcpListener, Router, Router>,
    pub port: u16,
}

impl Application {
    /// Build an axum app.
    ///
    /// # Panics
    /// Panics if the database is not reachable or if the port is already in use.
    ///
    /// # Errors
    /// Errors if the listen port or address is invalid.
    pub async fn build(
        database_url: &str,
        (listener_ip, listener_port): (&str, u16),
    ) -> Result<Self, std::io::Error> {
        let ApplicationRouter { router } = ApplicationRouter::build(database_url)
            .await
            .expect("database should be reachable");

        let listener = TcpListener::bind(format!("{listener_ip}:{listener_port}"))
            .await
            .unwrap_or_else(|_| {
                panic!("`{listener_ip}:{listener_port}` should not already be in use")
            });
        let port = listener.local_addr().unwrap().port();
        tracing::info!("App service listening on {}", listener.local_addr()?);

        Ok(Self {
            server: axum::serve(listener, router),
            port,
        })
    }

    /// Run the app.  Can be used in tests and when running the app in production.
    ///
    /// # Errors
    ///
    /// This function will return an error if the axum main server or metrics server returned an
    /// error.
    pub async fn run_until_stopped(
        self,
        opentelemetry_providers: Option<OpenTelemetryProviders>,
    ) -> Result<(), std::io::Error> {
        self.server
            .with_graceful_shutdown(shutdown_signal(opentelemetry_providers.clone()))
            .await?;

        Ok(())
    }
}

/// Create the main app axum router.
///
/// # Panics
/// Panics when not able to reach the database.
///
/// Panics if .
pub async fn router(database_url: &str) -> Router {
    tracing::info!("App service starting");

    let db_pool = SqlitePool::connect(database_url)
        .await
        .expect("SQLite database should be reachable");
    run_migrations(&db_pool).await;

    let schema = get_schema(db_pool);

    init_router(schema)
}
