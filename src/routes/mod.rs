use async_graphql::http::{GraphQLPlaygroundConfig, playground_source};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{Json, extract::Extension, http::StatusCode, response::Html};
use opentelemetry::trace::TraceContextExt;
use serde::Serialize;
use tracing::{Instrument, Level, span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

use crate::model::ServiceSchema;

#[derive(Serialize)]
pub(crate) struct Health {
    healthy: bool,
}

pub(crate) async fn health() -> (StatusCode, Json<Health>) {
    let health = Health { healthy: true };

    (StatusCode::OK, Json(health))
}

pub(crate) async fn graphql_playground() -> Html<String> {
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
