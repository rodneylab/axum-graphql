use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::Extension,
    http::StatusCode,
    response::{Html, IntoResponse},
    Json,
};
use opentelemetry::trace::TraceContextExt;
use serde::Serialize;
use tracing::{span, Instrument, Level};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::model::ServiceSchema;

#[derive(Serialize)]
struct Health {
    healthy: bool,
}

pub(crate) async fn health() -> impl IntoResponse {
    let health = Health { healthy: true };

    (StatusCode::OK, Json(health))
}

pub(crate) async fn graphql_playground() -> impl IntoResponse {
    Html(
        // serve GraphQL Playground CDN assets locally
        playground_source(GraphQLPlaygroundConfig::new("/").subscription_endpoint("/ws"))
            .replace(
                "//cdn.jsdelivr.net/npm/graphql-playground-react/build",
                "/assets",
            )
            .replace(
                "https://fonts.googleapis.com/css",
                "/assets/fonts/fonts.css",
            ),
    )
}

pub(crate) async fn graphql_handler(
    Extension(schema): Extension<ServiceSchema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let span = span!(Level::INFO, "graphql_execution");

    tracing::info!("Processing GraphQL request");

    let response = async move { schema.execute(req.into_inner()).await }
        .instrument(span.clone())
        .await;

    tracing::info!("Processing GraphQL request finished");

    response
        .extension(
            "traceId",
            async_graphql::Value::String(format!(
                "{}",
                span.context().span().span_context().trace_id()
            )),
        )
        .into()
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{header, Method, Request, StatusCode},
        Router,
    };
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use sqlx::sqlite::SqlitePoolOptions;
    use tower::util::ServiceExt;

    use crate::main_app;

    async fn get_app() -> Router {
        let database_url = "sqlite://:memory:";
        let app = main_app(database_url).await;

        let db_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(database_url)
            .await
            .unwrap();

        app.with_state(db_pool)
    }

    #[tokio::test]
    async fn graphql_endpoint_returns_200_ok() {
        // arrange
        let app = get_app().await;

        // act
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        // assert
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn graphql_endpoint_responds_to_invalid_query() {
        // arrange
        let app = get_app().await;
        let json_request_body: Value = json!(
        {"operationName":"HelloQuery","variables":{},"query":"query HelloQuery { hello "
            });

        // act
        let response = app
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
        let app = get_app().await;

        // act
        let response = app
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
}
