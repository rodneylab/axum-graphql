use metrics_exporter_prometheus::PrometheusHandle;
use once_cell::sync::Lazy;
use opentelemetry_sdk::trace::SdkTracerProvider;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

use axum_graphql::{
    database::run_migrations,
    observability::{
        metrics::create_prometheus_recorder, tracing::create_tracing_subscriber_from_env,
    },
    startup::{Application, ApplicationRouters},
};

pub static METRICS: Lazy<PrometheusHandle> = Lazy::new(create_prometheus_recorder);
static TRACING: Lazy<Option<SdkTracerProvider>> = Lazy::new(create_tracing_subscriber_from_env);

pub struct TestApp {
    pub main_server_port: u16,
    pub metrics_server_port: u16,
}

impl TestApp {
    pub async fn spawn() -> Self {
        let tracer_provider = Lazy::force(&TRACING);
        let database_url = "sqlite://:memory:";
        let recorder_handle = Lazy::<PrometheusHandle>::force(&METRICS).clone();

        let app = Application::build(
            database_url,
            recorder_handle,
            ("127.0.0.1", 0),
            ("127.0.0.1", 0),
        )
        .await
        .unwrap();

        let Application {
            main_server_port,
            metrics_server_port,
            ..
        } = app;

        #[expect(clippy::let_underscore_future)]
        let _ = tokio::spawn(app.run_until_stopped(tracer_provider.clone()));

        Self {
            main_server_port,
            metrics_server_port,
        }
    }

    pub async fn spawn_routers() -> ApplicationRouters {
        let database_url = "sqlite://:memory:";
        let recorder_handle = Lazy::<PrometheusHandle>::force(&METRICS).clone();

        ApplicationRouters::build(database_url, recorder_handle)
            .await
            .expect("database should be reachable")
    }

    /// Generates fresh in-memory `SQLite` database and runs migrations.  Can be called from
    /// each test.
    pub async fn get_db_pool() -> SqlitePool {
        let database_url = "sqlite://:memory:";

        let db_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(database_url)
            .await
            .unwrap();

        run_migrations(&db_pool).await;

        db_pool
    }
}
