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
