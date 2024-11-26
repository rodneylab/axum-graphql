use std::future::ready;

use axum::{extract::Extension, middleware, routing::get, serve::Serve, Router};
use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::SqlitePool;
use tokio::signal;
use tower_http::{compression::CompressionLayer, services::ServeDir, timeout::TimeoutLayer};

use crate::{
    database::run_migrations,
    model::get_schema,
    observability::metrics::track as track_metrics,
    routes::{graphql_handler, graphql_playground, health},
};

pub struct ApplicationRouters {
    pub main_router: Router,
    pub metrics_router: Router,
}

impl ApplicationRouters {
    /// Build the main app and metrics app routers.  These routers can be used in unit tests.
    ///
    /// # Errors
    /// Returns an error if the database is not reachable
    ///
    /// This function will return an error if .
    pub async fn build(
        database_url: &str,
        recorder_handle: PrometheusHandle,
    ) -> Result<Self, std::io::Error> {
        Ok(Self {
            main_router: main_router(database_url).await,
            metrics_router: metrics_router(recorder_handle),
        })
    }
}

/// Listen for and handle shutdown signals.
///
/// # Panics
///
/// Panics if unable to install Ctrl +C handler.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
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
            opentelemetry::global::shutdown_tracer_provider();
        },
        () = terminate => {
            tracing::info!("Terminate registered");
            opentelemetry::global::shutdown_tracer_provider();
        },
    }
}

pub struct Application {
    pub main_server: Serve<Router, Router>,
    pub main_server_port: u16,
    pub metrics_server: Serve<Router, Router>,
    pub metrics_server_port: u16,
}

impl Application {
    /// Build an axum main app and a metrics app.
    ///
    /// # Panics
    /// Panics if the database is not reachable, if the main app or merics port is already in use.
    ///
    /// # Errors
    ///
    /// This function will return an error if the metric port is already in use.
    pub async fn build(
        database_url: &str,
        recorder_handle: PrometheusHandle,
        (main_listener_ip, main_listener_port): (&str, u16),
        (metrics_listener_ip, metrics_listener_port): (&str, u16),
    ) -> Result<Self, std::io::Error> {
        let ApplicationRouters {
            main_router,
            metrics_router,
        } = ApplicationRouters::build(database_url, recorder_handle)
            .await
            .expect("database should be reachable");

        let main_listener =
            tokio::net::TcpListener::bind(format!("{main_listener_ip}:{main_listener_port}"))
                .await
                .unwrap_or_else(|_| {
                    panic!("`{main_listener_ip}:{main_listener_port}` should not already be in use")
                });
        let main_server_port = main_listener.local_addr().unwrap().port();
        tracing::info!(
            "Main app service listening on {}",
            main_listener.local_addr().unwrap()
        );

        let metrics_listener =
            tokio::net::TcpListener::bind(format!("{metrics_listener_ip}:{metrics_listener_port}"))
                .await?;
        let metrics_server_port = metrics_listener.local_addr().unwrap().port();

        tracing::info!(
            "Metrics service listening on {}",
            metrics_listener.local_addr().unwrap()
        );

        Ok(Self {
            main_server: axum::serve(main_listener, main_router),
            main_server_port,
            metrics_server: axum::serve(metrics_listener, metrics_router),
            metrics_server_port,
        })
    }

    /// Run both apps.  Can be used in tests and when running the app in production.
    ///
    /// # Errors
    ///
    /// This function will return an error if the axum main server or metrics server returned an
    /// error.
    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        let (main_server, metrics_server) = tokio::join!(
            self.main_server.with_graceful_shutdown(shutdown_signal()),
            self.metrics_server
                .with_graceful_shutdown(shutdown_signal())
        );
        main_server?;
        metrics_server?;

        Ok(())
    }
}

/// Create the main app axum router.
///
/// # Panics
/// Panics when not able to reach the database.
///
/// Panics if .
pub async fn main_router(database_url: &str) -> Router {
    tracing::info!("Main app service starting");

    let db_pool = SqlitePool::connect(database_url)
        .await
        .expect("SQLite database should be reachable");
    run_migrations(&db_pool).await;

    let schema = get_schema(db_pool);

    Router::new()
        .route("/", get(graphql_playground).post(graphql_handler))
        .route("/health", get(health))
        // serve GraphQL Playground CDN assets locally
        .nest_service("/assets", ServeDir::new("public"))
        .layer(CompressionLayer::new())
        .layer((
            middleware::from_fn(track_metrics),
            TimeoutLayer::new(std::time::Duration::from_secs(10)),
        ))
        .layer(Extension(schema))
}

pub fn metrics_router(recorder_handle: PrometheusHandle) -> Router {
    Router::new().route("/metrics", get(move || ready(recorder_handle.render())))
}
