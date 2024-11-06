mod post;

use async_graphql::{Context, EmptySubscription, Object, Schema};
use sqlx::SqlitePool;

use post::{
    create_draft_mutation, delete_draft_mutation, drafts_query, posts_query, publish_mutation,
    DeleteDraftResponse, Post, PublishResponse, ValidationError,
};

pub(crate) type ServiceSchema = Schema<QueryRoot, MutationRoot, EmptySubscription>;

pub(crate) fn get_schema(db_pool: SqlitePool) -> ServiceSchema {
    Schema::build(QueryRoot, MutationRoot, EmptySubscription)
        .data(db_pool)
        // .limit_complexity(20) // may impact GraphQL Playground documentation
        // .limit_depth(5) // may impact GraphQL Playground documentation
        // Registering ValidationError manually as it is not currently directly referenced
        .register_output_type::<ValidationError>()
        .finish()
}

pub(crate) struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Returns a welcoming greeting
    #[graphql(cache_control(max_age = 3600))]
    async fn hello(&self, _ctx: &Context<'_>) -> &'static str {
        "Hello everybody!"
    }

    /// Returns a list of draft posts
    async fn drafts(&self, ctx: &Context<'_>) -> Result<Vec<Post>, anyhow::Error> {
        let db_pool = ctx.data_unchecked::<SqlitePool>();

        drafts_query(db_pool).await
    }

    /// Returns a list of published posts
    async fn posts(&self, ctx: &Context<'_>) -> Result<Vec<Post>, anyhow::Error> {
        let db_pool = ctx.data_unchecked::<SqlitePool>();

        posts_query(db_pool).await
    }
}

pub(crate) struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Creates a new draft with `title` and `body`
    async fn create_draft(
        &self,
        ctx: &Context<'_>,
        #[graphql(validator(min_length = 3, max_length = 64))] title: String,
        #[graphql(validator(min_length = 3, max_length = 64_000))] body: String,
    ) -> Result<Post, anyhow::Error> {
        let db_pool = ctx.data_unchecked::<SqlitePool>();

        create_draft_mutation(db_pool, &title, &body).await
    }

    /// Deletes the draft post with `id`
    async fn delete_draft(
        &self,
        ctx: &Context<'_>,
        #[graphql(validator(minimum = 0))] id: i64,
    ) -> Result<DeleteDraftResponse, anyhow::Error> {
        let db_pool = ctx.data_unchecked::<SqlitePool>();

        delete_draft_mutation(db_pool, id).await
    }

    /// Updates `published` field for post with `id` to `true`
    async fn publish(
        &self,
        ctx: &Context<'_>,
        #[graphql(validator(minimum = 0))] id: i64,
    ) -> Result<PublishResponse, anyhow::Error> {
        let db_pool = ctx.data_unchecked::<SqlitePool>();

        publish_mutation(db_pool, id).await
    }
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
    async fn graphql_endpoint_responds_to_hello_query() {
        // arrange
        let app = get_app().await;
        let json_request_body: Value = json!({
            "operationName":"HelloQuery",
            "variables":{},
            "query":"query HelloQuery { hello }"
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
                "data": { "hello": "Hello everybody!" },
                "extensions": {"traceId": "00000000000000000000000000000000"}
            })
        );
    }

    #[tokio::test]
    async fn graphql_endpoint_responds_to_drafts_query() {
        // arrange
        let app = get_app().await;
        let drafts_json_request_body: Value = json!({
            "operationName":"DraftsQuery",
            "variables":{},
            "query":"query DraftsQuery { drafts { id title} }"
        });

        // act
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/")
                    .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(drafts_json_request_body.to_string()))
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
                "data": { "drafts": [] },
                "extensions": {"traceId": "00000000000000000000000000000000"}
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
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/")
                    .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(create_draft_json_request_body.to_string()))
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
                "data": { "createDraft": { "id": 1, "title": "Draft title" } },
                "extensions": {"traceId": "00000000000000000000000000000000"}
            })
        );

        // arrange
        // act
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/")
                    .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::from(drafts_json_request_body.to_string()))
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
                "data": { "drafts": [ { "id": 1, "title": "Draft title" } ] },
                "extensions": {"traceId": "00000000000000000000000000000000"}
            })
        );
    }
}
