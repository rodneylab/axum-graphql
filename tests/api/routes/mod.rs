use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use axum_graphql::startup::ApplicationRouter;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::util::ServiceExt;

use crate::helpers::TestApp;

#[tokio::test]
async fn graphql_endpoint_returns_200_ok() {
    // arrange
    let ApplicationRouter { router, .. } = TestApp::spawn_routers().await;

    // act
    let response = router
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    // assert
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn graphql_endpoint_responds_to_invalid_query() {
    // arrange
    let ApplicationRouter { router, .. } = TestApp::spawn_routers().await;
    let json_request_body: Value = json!(
    {"operationName":"HelloQuery","variables":{},"query":"query HelloQuery { hello "
        });

    // act
    let response = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/")
                .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .body(Body::from(json_request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // assert
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body,
        json!({
            "data": None::<String>,
            "errors": [{
                "locations": [{"column": 26,"line": 1}],
                "message": " --> 1:26\n  |\n1 | query HelloQuery { hello \n  |                          ^---\n  |\n  = expected selection_set, selection, directive, or arguments"
            }],
            "extensions": {"traceId": "00000000000000000000000000000000"}
        })
    );
}

#[tokio::test]
async fn health_check_returns_expected_json_response_with_200_ok() {
    // arrange
    let ApplicationRouter { router, .. } = TestApp::spawn_routers().await;

    // act
    let response = router
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // assert
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body, json!({ "healthy": true }));
}
