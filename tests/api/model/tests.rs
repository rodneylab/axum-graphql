use std::{path::Path, str};

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use axum_graphql::startup::ApplicationRouter;
use futures::executor::block_on;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::{Service, ServiceExt};

use crate::helpers::TestApp;

mod helpers {
    use axum::{
        Router,
        body::Body,
        http::{Method, Request, header},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::{Service, ServiceExt};

    pub async fn create_draft(app: &mut Router, title: &str, body: &str) -> i64 {
        let create_draft_json_request_body: Value = json!({
            "operationName":"CreateDraftMutation",
            "variables":{},
            "query": format!(r#"mutation CreateDraftMutation {{
                 createDraft(title: "{title}", body: "{body}") {{
                     id
                     title
                 }}
            }}"#),
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/")
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(create_draft_json_request_body.to_string()))
            .unwrap();
        let response = ServiceExt::<Request<Body>>::ready(app)
            .await
            .unwrap()
            .call(request)
            .await
            .unwrap();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json_data: Value = serde_json::from_slice(&body).unwrap();
        let id = &json_data["data"]["createDraft"]["id"];

        id.as_i64().unwrap()
    }

    pub async fn publish_draft(app: &mut Router, id: i64) {
        let publish_draft_json_request_body: Value = json!({
            "operationName":"PublishMutation",
            "variables":{},
            "query": format!("mutation PublishMutation {{
  publish(id: {id}) {{
    __typename
    ... on PublishSuccessResponse {{
      post {{
        id
        published
      }}
    }}
    ... on PublishErrorResponse {{
      error {{
        field
        message
        received
      }}
    }}
  }}
}}"),
        });

        let request = Request::builder()
            .method(Method::POST)
            .uri("/")
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(publish_draft_json_request_body.to_string()))
            .unwrap();
        let _response = ServiceExt::<Request<Body>>::ready(app)
            .await
            .unwrap()
            .call(request)
            .await
            .unwrap();
    }
}

async fn snapshot_graqphql_query_async<P: AsRef<Path>>(path: P) {
    // arrange
    let ApplicationRouter { router } = TestApp::spawn_routers().await;
    let json_request_body: Value = serde_json::from_slice(
        &std::fs::read(&path).expect("file should exist and have read permissions set"),
    )
    .unwrap_or_else(|_| {
        panic!(
            "File `{}` should contain valid JSON",
            path.as_ref().display()
        )
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
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = str::from_utf8(&body_bytes).unwrap();
    let body_json: Value = serde_json::from_str(body).unwrap();
    insta::assert_json_snapshot!(body_json, {".extensions.traceId" => "[traceId]"});
    // assert_eq!(
    //     body_json["extensions"]["traceId"],
    //     "00000000000000000000000000000000"
    // );
}

#[tokio::test]
async fn snapshot_graphql_queries() {
    insta::glob!("snapshot_inputs/*.json", |path| {
        block_on(snapshot_graqphql_query_async(path));
    });
}

#[tokio::test]
async fn graphql_endpoint_responds_to_hello_query() {
    // arrange
    let ApplicationRouter { router } = TestApp::spawn_routers().await;
    let json_request_body: Value = json!({
        "operationName":"HelloQuery",
        "variables":{},
        "query":"query HelloQuery { hello }"
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
            "data": { "hello": "Hello everybody!" },
            "extensions": { "traceId": "00000000000000000000000000000000" }
        })
    );
}

#[tokio::test]
async fn graphql_endpoint_responds_to_drafts_query() {
    // arrange
    let ApplicationRouter { mut router } = TestApp::spawn_routers().await;
    let drafts_json_request_body: Value = json!({
        "operationName":"DraftsQuery",
        "variables":{},
        "query":"query DraftsQuery { drafts { id title} }"
    });

    // act
    let request = Request::builder()
        .method(Method::POST)
        .uri("/")
        .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        .body(Body::from(drafts_json_request_body.to_string()))
        .unwrap();
    let response = ServiceExt::<Request<Body>>::ready(&mut router)
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap();

    // assert
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body,
        json!({
            "data": { "drafts": [] },
            "extensions": { "traceId": "00000000000000000000000000000000" }
        })
    );

    // arrange
    let create_draft_json_request_body: Value = json!({
        "operationName":"CreateDraftMutation",
        "variables":{},
        "query": r#"mutation CreateDraftMutation {
                 createDraft(title: "Draft title", body: "Draft body text") {
                     id
                     title
                 }
            }"#,
    });

    // act
    let request = Request::builder()
        .method(Method::POST)
        .uri("/")
        .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        .body(Body::from(create_draft_json_request_body.to_string()))
        .unwrap();
    let response = ServiceExt::<Request<Body>>::ready(&mut router)
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap();

    // assert
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body,
        json!({
            "data": { "createDraft": { "id": 1, "title": "Draft title" } },
            "extensions": { "traceId": "00000000000000000000000000000000" }
        })
    );

    // act
    let request = Request::builder()
        .method(Method::POST)
        .uri("/")
        .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        .body(Body::from(drafts_json_request_body.to_string()))
        .unwrap();
    let response = ServiceExt::<Request<Body>>::ready(&mut router)
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap();

    // assert
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body,
        json!({
            "data": { "drafts": [ { "id": 1, "title": "Draft title" } ] },
            "extensions": { "traceId": "00000000000000000000000000000000" }
        })
    );
}

#[tokio::test]
async fn posts_return_empty_array_when_no_posts_exist() {
    // arrange
    let ApplicationRouter { router } = TestApp::spawn_routers().await;
    let posts_json_request_body: Value = json!({
        "operationName":"PostsQuery",
        "variables":{},
        "query":"query PostsQuery { posts { id title} }"
    });

    // act
    let response = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/")
                .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .body(Body::from(posts_json_request_body.to_string()))
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
            "data": { "posts": [] },
            "extensions": { "traceId": "00000000000000000000000000000000" }
        })
    );
}

#[tokio::test]
async fn posts_returns_existing_posts() {
    // arrange
    let ApplicationRouter { mut router } = TestApp::spawn_routers().await;
    let id_1 = helpers::create_draft(&mut router, "First Post Title", "First post body.").await;
    let _id_2 = helpers::create_draft(&mut router, "Second Post Title", "Second post body.").await;
    let id_3 = helpers::create_draft(&mut router, "Third Post Title", "Third post body.").await;
    helpers::publish_draft(&mut router, id_1).await;
    helpers::publish_draft(&mut router, id_3).await;
    let posts_json_request_body: Value = json!({
        "operationName":"PostsQuery",
        "variables":{},
        "query":"query PostsQuery { posts { id title} }"
    });

    // act
    let request = Request::builder()
        .method(Method::POST)
        .uri("/")
        .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
        .body(Body::from(posts_json_request_body.to_string()))
        .unwrap();
    let response = ServiceExt::<Request<Body>>::ready(&mut router)
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap();

    // assert
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        body,
        json!({
            "data": { "posts": [
                { "id": id_1, "title": "First Post Title" },
                { "id": id_3, "title": "Third Post Title" },
            ]},
            "extensions": { "traceId": "00000000000000000000000000000000" }
        })
    );
}

#[tokio::test]
async fn publish_returns_user_error_for_invalid_id() {
    // arrange
    let ApplicationRouter { router } = TestApp::spawn_routers().await;
    let id = 9_999;
    let publish_draft_json_request_body: Value = json!({
        "operationName":"PublishMutation",
        "variables":{},
        "query": format!("mutation PublishMutation {{
  publish(id: {id}) {{
    __typename
    ... on PublishSuccessResponse {{
      post {{
        id
        published
      }}
    }}
    ... on PublishErrorResponse {{
      error {{
        field
        message
        received
      }}
    }}
  }}
}}"),
    });

    // act
    let response = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/")
                .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .body(Body::from(publish_draft_json_request_body.to_string()))
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
            "data": { "publish": {
                "__typename": "PublishErrorResponse",
                "error": {
                    "field": "id",
                    "message": "Did not find draft post with id `9999`",
                    "received": "9999"
                }
            }},
            "extensions": { "traceId": "00000000000000000000000000000000" }
        })
    );
}

#[tokio::test]
async fn publish_returns_user_expected_result_for_valid_input() {
    // arrange
    let ApplicationRouter { mut router } = TestApp::spawn_routers().await;
    let _id_1 = helpers::create_draft(&mut router, "First Post Title", "First post body.").await;
    let id_2 = helpers::create_draft(&mut router, "Second Post Title", "Second post body.").await;
    let _id_3 = helpers::create_draft(&mut router, "Third Post Title", "Third post body.").await;
    let publish_draft_json_request_body: Value = json!({
        "operationName":"PublishMutation",
        "variables":{},
        "query": format!("mutation PublishMutation {{
  publish(id: {id_2}) {{
    __typename
    ... on PublishSuccessResponse {{
      post {{
        id
        published
      }}
    }}
    ... on PublishErrorResponse {{
      error {{
        field
        message
        received
      }}
    }}
  }}
}}"),
    });

    // act
    let response = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/")
                .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .body(Body::from(publish_draft_json_request_body.to_string()))
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
            "data": { "publish": {
                "__typename": "PublishSuccessResponse",
                "post": {
                    "id": id_2,
                    "published": true,
                }
            }},
            "extensions": { "traceId": "00000000000000000000000000000000" }
        })
    );
}

#[tokio::test]
async fn delete_draft_returns_user_error_for_invalid_id() {
    // arrange
    let ApplicationRouter { router } = TestApp::spawn_routers().await;
    let id = 9_999;
    let delete_draft_json_request_body: Value = json!({
        "operationName":"DeleteDraftMutation",
        "variables":{},
        "query": format!("mutation DeleteDraftMutation {{
  deleteDraft(id: {id}) {{
    __typename
    ... on DeleteDraftSuccessResponse {{
      post {{
        id
        title
      }}
    }}
    ... on DeleteDraftErrorResponse {{
      error {{
        field
        message
        received
      }}
    }}
  }}
}}"),
    });

    // act
    let response = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/")
                .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .body(Body::from(delete_draft_json_request_body.to_string()))
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
            "data": { "deleteDraft": {
                "__typename": "DeleteDraftErrorResponse",
                "error": {
                    "field": "id",
                    "message": "Did not find draft post with id `9999`",
                    "received": "9999"
                }
            }},
            "extensions": { "traceId": "00000000000000000000000000000000" }
        })
    );
}

#[tokio::test]
async fn delete_draft_returns_user_expected_result_for_valid_input() {
    // arrange
    let ApplicationRouter { mut router } = TestApp::spawn_routers().await;
    let _id_1 = helpers::create_draft(&mut router, "First Post Title", "First post body.").await;
    let id_2 = helpers::create_draft(&mut router, "Second Post Title", "Second post body.").await;
    let _id_3 = helpers::create_draft(&mut router, "Third Post Title", "Third post body.").await;
    let delete_draft_json_request_body: Value = json!({
        "operationName":"DeleteDraftMutation",
        "variables":{},
        "query": format!("mutation DeleteDraftMutation {{
  deleteDraft(id: {id_2}) {{
    __typename
    ... on DeleteDraftSuccessResponse {{
      post {{
        id
        title
      }}
    }}
    ... on DeleteDraftErrorResponse {{
      error {{
        field
        message
        received
      }}
    }}
  }}
}}"),
    });

    // act
    let response = router
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/")
                .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                .body(Body::from(delete_draft_json_request_body.to_string()))
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
            "data": { "deleteDraft": {
                "__typename": "DeleteDraftSuccessResponse",
                "post": {
                    "id": id_2,
                    "title": "Second Post Title"
                }
            }},
            "extensions": { "traceId": "00000000000000000000000000000000" }
        })
    );
}
