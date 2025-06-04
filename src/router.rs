use std::sync::Arc;

use axum::{
    BoxError, Extension, Router, error_handling::HandleErrorLayer, http::StatusCode, middleware,
    routing::get,
};
use tower::{ServiceBuilder, timeout::TimeoutLayer};
use tower_http::{compression::CompressionLayer, services::ServeDir};

use crate::{
    model::ServiceSchema,
    observability::metrics::{self, AppMetricsState},
    routes::{graphql_handler, graphql_playground, health},
};

#[derive(Clone)]
pub struct AppState {
    pub metrics: AppMetricsState,
}

pub(crate) fn init_router(schema: ServiceSchema) -> Router {
    let state = AppState {
        metrics: AppMetricsState::default(),
    };
    let shared_state = Arc::new(state);

    Router::new()
        .route("/", get(graphql_playground).post(graphql_handler))
        .route("/health", get(health))
        // serve GraphQL Playground CDN assets locally
        .nest_service("/assets", ServeDir::new("public"))
        .layer(
            ServiceBuilder::new()
                .layer(Extension(schema))
                .layer(CompressionLayer::new())
                .layer(HandleErrorLayer::new(|_: BoxError| async {
                    StatusCode::REQUEST_TIMEOUT
                }))
                .layer(TimeoutLayer::new(std::time::Duration::from_secs(15)))
                .layer(middleware::from_fn_with_state(
                    Arc::clone(&shared_state),
                    metrics::track,
                )),
        )
        .with_state(shared_state)
}
