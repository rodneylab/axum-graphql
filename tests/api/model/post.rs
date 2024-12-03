use crate::helpers::TestApp;
use axum_graphql::model::post::{
    create_draft_mutation, delete_draft_mutation, posts_query, publish_mutation,
    DeleteDraftErrorResponse, DeleteDraftResponse, DeleteDraftSuccessResponse, Post,
    UserInputError,
};
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn posts_query_returns_expected_output_with_no_posts() {
    // arrange
    let db_pool = TestApp::get_db_pool().await;

    // act
    let result = posts_query(&db_pool).await.unwrap();

    // assert
    assert_eq!(result, Vec::<Post>::new());
}

#[tokio::test]
async fn posts_query_returns_expected_output_with_posts() {
    // arrange
    let db_pool = TestApp::get_db_pool().await;
    let title = String::from("New Post Title");
    let body = String::from("# New Post\nNew post body");
    let Post { id, .. } = create_draft_mutation(&db_pool, &title, &body)
        .await
        .unwrap();
    let _ = publish_mutation(&db_pool, id).await;

    // act
    let result = posts_query(&db_pool).await.unwrap();

    // assert
    assert_eq!(
        result,
        vec![Post {
            id,
            title,
            body,
            published: true
        }]
    );
}

#[tokio::test]
async fn create_draft_mutation_fails_if_db_is_not_initialised() {
    // arrange
    let database_url = "sqlite://:memory:";

    // create database pool manually and omit migrations
    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .unwrap();

    // act
    let outcome = create_draft_mutation(&db_pool, "Draft Post Title", "Draft Post Body")
        .await
        .unwrap_err();

    // assert
    assert_eq!(format!("{outcome}"), "run create draft mutation for post");
    let mut chain = outcome.chain();
    assert_eq!(
        chain.next().map(|val| format!("{val}")),
        Some(String::from("run create draft mutation for post"))
    );
    assert_eq!(
        chain.next().map(|val| format!("{val}")),
        Some(String::from(
            "error returned from database: (code: 1) no such table: Post"
        ))
    );
    assert_eq!(
        chain.next().map(|val| format!("{val}")),
        Some(String::from("(code: 1) no such table: Post"))
    );
    assert_eq!(chain.next().map(|val| format!("{val}")), None);
}

#[tokio::test]
async fn delete_draft_mutation_returns_error_message_if_draft_does_not_exist() {
    // arrange
    let db_pool = TestApp::get_db_pool().await;
    let title = String::from("New Post Title");
    let body = String::from("# New Post\nNew post body");
    let Post { id, .. } = create_draft_mutation(&db_pool, &title, &body)
        .await
        .unwrap();
    let _ = publish_mutation(&db_pool, id).await;

    // act
    let outcome = delete_draft_mutation(&db_pool, 999).await.unwrap();

    // assert
    assert_eq!(
        outcome,
        DeleteDraftResponse::DeleteDraftErrorResponse(DeleteDraftErrorResponse {
            error: UserInputError {
                field: String::from("id"),
                message: String::from("Did not find draft post with id `999`"),
                received: String::from("999")
            }
        })
    );
}

#[tokio::test]
async fn delete_draft_mutation_returns_draft_on_valid_input() {
    // arrange
    let db_pool = TestApp::get_db_pool().await;
    let title = String::from("New Post Title");
    let body = String::from("# New Post\nNew post body");
    let Post { id, .. } = create_draft_mutation(&db_pool, &title, &body)
        .await
        .unwrap();

    // act
    let outcome = delete_draft_mutation(&db_pool, id).await.unwrap();

    // assert
    assert_eq!(
        outcome,
        DeleteDraftResponse::DeleteDraftSuccessResponse(DeleteDraftSuccessResponse {
            post: Post {
                id,
                title,
                body,
                published: false
            },
        })
    );
}

#[tokio::test]
async fn delete_draft_mutation_fails_if_db_is_not_initialised() {
    // arrange
    let database_url = "sqlite://:memory:";

    // create database pool manually and omit migrations
    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .unwrap();

    // act
    let outcome = delete_draft_mutation(&db_pool, 9_999).await.unwrap_err();

    // assert
    assert_eq!(
        format!("{outcome}"),
        "error returned from database: (code: 1) no such table: Post"
    );
    let mut chain = outcome.chain();
    assert_eq!(
        chain.next().map(|val| format!("{val}")),
        Some(String::from(
            "error returned from database: (code: 1) no such table: Post"
        ))
    );
    assert_eq!(
        chain.next().map(|val| format!("{val}")),
        Some(String::from("(code: 1) no such table: Post"))
    );
    assert_eq!(chain.next().map(|val| format!("{val}")), None);
}

#[tokio::test]
async fn publish_mutation_fails_if_db_is_not_initialised() {
    // arrange
    let database_url = "sqlite://:memory:";

    // create database pool manually and omit migrations
    let db_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .unwrap();

    // act
    let outcome = publish_mutation(&db_pool, 99_999).await.unwrap_err();

    // assert
    assert_eq!(
        format!("{outcome}"),
        "error returned from database: (code: 1) no such table: Post"
    );
    let mut chain = outcome.chain();
    assert_eq!(
        chain.next().map(|val| format!("{val}")),
        Some(String::from(
            "error returned from database: (code: 1) no such table: Post"
        ))
    );
    assert_eq!(
        chain.next().map(|val| format!("{val}")),
        Some(String::from("(code: 1) no such table: Post"))
    );
    assert_eq!(chain.next().map(|val| format!("{val}")), None);
}
