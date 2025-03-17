use once_cell::sync::Lazy;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

use axum_graphql::{
    database::run_migrations,
    observability::{OpenTelemetryProviders, initialise_observability},
    startup::{Application, ApplicationRouter},
};

static TRACING: Lazy<Option<OpenTelemetryProviders>> = Lazy::new(initialise_observability);

pub struct TestApp {
    pub port: u16,
}

impl TestApp {
    pub async fn spawn() -> Self {
        unsafe {
            std::env::set_var("OPENTELEMETRY_ENABLED", "true");
        }
        let tracer_provider = Lazy::force(&TRACING);
        let database_url = "sqlite://:memory:";

        let app = Application::build(database_url, ("127.0.0.1", 0))
            .await
            .unwrap();

        let Application { port, .. } = app;

        #[expect(clippy::let_underscore_future)]
        let _ = tokio::spawn(app.run_until_stopped(tracer_provider.clone()));

        Self { port }
    }

    pub async fn spawn_routers() -> ApplicationRouter {
        let database_url = "sqlite://:memory:";

        ApplicationRouter::build(database_url)
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
