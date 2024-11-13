use std::future::ready;

use axum::{extract::Extension, middleware, routing::get, serve::Serve, Router};
use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::SqlitePool;
use tokio::signal;
use tower_http::{compression::CompressionLayer, services::ServeDir, timeout::TimeoutLayer};

use crate::{
    database::run_migrations,
    model::get_schema,
    observability::metrics::track_metrics,
    routes::{graphql_handler, graphql_playground, health},
};

pub struct Application {
    main_server: Serve<Router, Router>,
    metrics_server: Serve<Router, Router>,
}

impl Application {
    pub async fn build(
        database_url: &str,
        recorder_handle: PrometheusHandle,
    ) -> Result<Self, std::io::Error> {
        let local_addr = "127.0.0.1:8000";
        let main_listener = tokio::net::TcpListener::bind(local_addr)
            .await
            .unwrap_or_else(|_| panic!("`{}` should not already be in use", &local_addr));
        tracing::info!(
            "Main app service listening on {}",
            main_listener.local_addr().unwrap()
        );
        let main_server = run_main_server(main_listener, database_url).await;

        let metrics_listener = tokio::net::TcpListener::bind("127.0.0.1:8001")
            .await
            .unwrap();
        tracing::info!(
            "Metrics service listening on {}",
            metrics_listener.local_addr().unwrap()
        );
        let metrics_server = run_metrics_server(metrics_listener, recorder_handle);

        Ok(Self {
            main_server,
            metrics_server,
        })
    }

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

async fn shutdown_signal() {
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

pub async fn run_main_server(
    listener: tokio::net::TcpListener,
    database_url: &str,
) -> Serve<Router, Router> {
    let router = main_router(database_url).await;

    axum::serve(listener, router)
}

pub fn run_metrics_server(
    listener: tokio::net::TcpListener,
    recorder_handle: PrometheusHandle,
) -> Serve<Router, Router> {
    let router = metrics_router(recorder_handle);

    axum::serve(listener, router)
}
