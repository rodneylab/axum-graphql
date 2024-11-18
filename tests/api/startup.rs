use std::thread;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
// use enigo::{
//     Direction::{Click, Press, Release},
//     Enigo, Key, Keyboard,
// };
use metrics_exporter_prometheus::PrometheusHandle;
use once_cell::sync::Lazy;
use reqwest::Client;
use tower::ServiceExt;

use crate::helpers::{TestApp, METRICS};
use axum_graphql::startup::ApplicationRouters;

#[tokio::test]
async fn application_router_build_successfully_creates_main_and_metrics_routers() {
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
async fn application_build_successfully_creates_main_and_metrics_servers() {
    // arrange
    std::env::set_var("RUST_TEST_NOCAPTURE", "true");
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    };
    let client = Client::builder()
        .timeout(std::time::Duration::from_millis(1_000))
        .build()
        .unwrap();

    // act
    let TestApp {
        main_server_port,
        metrics_server_port,
    } = TestApp::spawn().await;
    let main_server_response = client
        .get(format!("http://localhost:{main_server_port}"))
        .send()
        .await
        .unwrap();
    let metrics_server_response = client
        .get(format!("http://localhost:{metrics_server_port}/metrics"))
        .send()
        .await
        .unwrap();

    // assert
    assert_eq!(main_server_response.status(), StatusCode::OK);
    assert_eq!(metrics_server_response.status(), StatusCode::OK);
}

// #[tokio::test]
// async fn application_registers_ctrl_c() {
//     // arrange
//     std::env::set_var("RUST_TEST_NOCAPTURE", "true");
//     if std::env::var("RUST_LOG").is_err() {
//         std::env::set_var("RUST_LOG", "info");
//     };
//     let client = Client::builder()
//         .timeout(std::time::Duration::from_millis(1_000))
//         .build()
//         .unwrap();

//     let TestApp {
//         main_server_port,
//         metrics_server_port,
//     } = TestApp::spawn().await;

//     // act
//     let mut enigo = Enigo::new(&enigo::Settings::default()).unwrap();
//     let _ = enigo.key(Key::Control, Press);
//     let _ = enigo.key(Key::Unicode('c'), Click);
//     let _ = enigo.key(Key::Control, Release);

//     thread::sleep(std::time::Duration::from_secs(30));

//     let main_server_error = client
//         .get(format!("http://localhost:{main_server_port}"))
//         .send()
//         .await
//         .unwrap_err();
//     let metrics_server_error = client
//         .get(format!("http://localhost:{metrics_server_port}/metrics"))
//         .send()
//         .await
//         .unwrap_err();

//     // assert
//     assert!(main_server_error.is_connect());
//     assert!(metrics_server_error.is_connect());
// }
