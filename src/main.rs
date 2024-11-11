#![warn(clippy::all, clippy::pedantic)]

mod database;
mod model;
mod observability;
mod routes;

use std::{env, future::ready};

use axum::{extract::Extension, middleware, routing::get, serve, Router};
use dotenvy::dotenv;
use metrics_exporter_prometheus::PrometheusHandle;
use sqlx::SqlitePool;
use tokio::signal;
use tower_http::{compression::CompressionLayer, services::ServeDir, timeout::TimeoutLayer};
use tracing::info;

use database::{create as create_database, run_migrations};
use model::get_schema;
use observability::{
    metrics::{create_prometheus_recorder, track_metrics},
    tracing::create_tracing_subscriber_from_env,
};
use routes::{graphql_handler, graphql_playground, health};

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

async fn main_app(database_url: &str) -> Router<SqlitePool> {
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

pub fn metrics_app(recorder_handle: PrometheusHandle) -> Router {
    info!("Metrics service starting");
    Router::new().route("/metrics", get(move || ready(recorder_handle.render())))
}

async fn start_main_server(database_url: &str) {
    create_tracing_subscriber_from_env();
    let app = main_app(database_url).await;
    let local_addr = "127.0.0.1:8000";
    let listener = tokio::net::TcpListener::bind(local_addr)
        .await
        .unwrap_or_else(|_| panic!("`{}` should not already be in use", &local_addr));
    tracing::info!(
        "Main app service listening on {}",
        listener.local_addr().unwrap()
    );

    let db_pool = SqlitePool::connect(database_url)
        .await
        .expect("SQLite database should be reachable");

    serve(listener, app.with_state(db_pool))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn start_metrics_server() {
    let recorder_handle = create_prometheus_recorder();
    let app = metrics_app(recorder_handle);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8001")
        .await
        .unwrap();
    tracing::info!(
        "Metrics service listening on {}",
        listener.local_addr().unwrap()
    );
    serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://sqlite.db".into());
    create_database(&database_url).await;

    let (_main_server, _metrics_server) =
        tokio::join!(start_main_server(&database_url), start_metrics_server());
}
