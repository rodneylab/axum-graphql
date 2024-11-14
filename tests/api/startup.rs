use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use metrics_exporter_prometheus::PrometheusHandle;
use once_cell::sync::Lazy;
use reqwest::Client;
use tower::ServiceExt;

use crate::helpers::METRICS;

use axum_graphql::{
    observability::tracing::create_tracing_subscriber_from_env,
    startup::{Application, ApplicationRouters},
};

#[tokio::test]
async fn aplication_router_build_successfully_creates_main_and_metrics_routers() {
    // arrange
    let database_url = "sqlite://:memory:";
    let recorder_handle: PrometheusHandle = Lazy::<PrometheusHandle>::force(&METRICS).clone();

    // act
    let routers = ApplicationRouters::build(database_url, recorder_handle.clone())
        .await
        .unwrap();
    let ApplicationRouters {
        main_router,
        metrics_router,
    } = routers;
    let main_server_response = main_router
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let metrics_server_response = metrics_router
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();

    // assert
    assert_eq!(main_server_response.status(), StatusCode::OK);
    assert_eq!(metrics_server_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn aplication_build_successfully_creates_main_and_metrics_servers() {
    // arrange
    let database_url = "sqlite://:memory:";
    let recorder_handle: PrometheusHandle = Lazy::<PrometheusHandle>::force(&METRICS).clone();
    let client = Client::builder()
        .timeout(std::time::Duration::from_millis(1_000))
        .build()
        .unwrap();

    // act

    let app = Application::build(database_url, recorder_handle.clone())
        .await
        .unwrap();

    #[expect(clippy::let_underscore_future)]
    let _ = tokio::spawn(app.run_until_stopped());

    let main_server_response = client.get("http://localhost:8000").send().await.unwrap();
    let metrics_server_response = client
        .get("http://localhost:8001/metrics")
        .send()
        .await
        .unwrap();

    // assert
    assert_eq!(main_server_response.status(), StatusCode::OK);
    assert_eq!(metrics_server_response.status(), StatusCode::OK);
}
