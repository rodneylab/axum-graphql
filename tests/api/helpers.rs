use metrics_exporter_prometheus::PrometheusHandle;
use once_cell::sync::Lazy;

use axum_graphql::{
    observability::{
        metrics::create_prometheus_recorder, tracing::create_tracing_subscriber_from_env,
    },
    startup::Application,
};

pub static METRICS: Lazy<PrometheusHandle> = Lazy::new(create_prometheus_recorder);
static TRACING: Lazy<()> = Lazy::new(|| {
    create_tracing_subscriber_from_env();
});

pub struct TestApp {
    pub main_server_port: u16,
    pub metrics_server_port: u16,
}

impl TestApp {
    pub async fn spawn() -> Self {
        Lazy::force(&TRACING);
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
        let _ = tokio::spawn(app.run_until_stopped());

        Self {
            main_server_port,
            metrics_server_port,
        }
    }
}
