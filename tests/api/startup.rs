use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use reqwest::Client;
use tower::ServiceExt;

use crate::helpers::TestApp;
use axum_graphql::startup::ApplicationRouter;

#[tokio::test]
async fn application_router_build_successfully_creates_main_and_metrics_routers() {
    // arrange
    let database_url = "sqlite://:memory:";

    // act
    let routers = ApplicationRouter::build(database_url).await.unwrap();
    let ApplicationRouter { router } = routers;
    let main_server_response = router
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    // assert
    assert_eq!(main_server_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn application_build_successfully_creates_main_and_metrics_servers() {
    // arrange
    unsafe {
        std::env::set_var("RUST_TEST_NOCAPTURE", "true");
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "info");
        }
    }
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .build()
        .unwrap();

    // act
    let TestApp { port } = TestApp::spawn().await;
    let main_server_response = client
        .get(format!("http://localhost:{port}"))
        .send()
        .await
        .unwrap();

    // assert
    assert_eq!(main_server_response.status(), StatusCode::OK);
}
