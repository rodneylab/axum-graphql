use anyhow::Context;
use async_graphql::{Interface, SimpleObject, Union};
use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(Debug, Deserialize, PartialEq, SimpleObject)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub published: bool,
}

#[derive(Debug, PartialEq, SimpleObject)]
/// Detail of the user input error
pub struct UserInputError {
    /// Field which had the invalid data
    pub field: String,

    /// Error description
    pub message: String,

    /// Input value for field
    pub received: String,
}

/// Errors generated while parsing or validating input parameters
#[derive(Interface)]
#[graphql(field(name = "message", ty = "String"))]
pub enum ValidationError {
    /// User input error, such as requesting an operation on a post with an `id` that does
    /// not exist
    UserInputError(UserInputError),
}

/// Response sent on valid delete draft mutation
#[derive(Debug, PartialEq, SimpleObject)]
pub struct DeleteDraftSuccessResponse {
    pub post: Post,
}

/// Response sent on delete draft mutation with validation issues identified
#[derive(Debug, PartialEq, SimpleObject)]
pub struct DeleteDraftErrorResponse {
    pub error: UserInputError,
}

/// Union of responses sent on delete draft mutation
#[derive(Debug, PartialEq, Union)]
pub enum DeleteDraftResponse {
    DeleteDraftSuccessResponse(DeleteDraftSuccessResponse),
    DeleteDraftErrorResponse(DeleteDraftErrorResponse),
}

/// Response sent on valid publish draft mutation
#[derive(Debug, PartialEq, SimpleObject)]
pub struct PublishSuccessResponse {
    /// Published post
    post: Post,
}

/// Response sent on publish draft mutation with validation issues identified
#[derive(Debug, PartialEq, SimpleObject)]
pub struct PublishErrorResponse {
    /// User input error details
    error: UserInputError,
}

/// Union of responses sent on publish draft mutation
#[derive(Debug, PartialEq, Union)]
pub enum PublishResponse {
    PublishSuccessResponse(PublishSuccessResponse),
    PublishErrorResponse(PublishErrorResponse),
}

/// Return a list of up to 100 draft posts
///
/// # Errors
///
/// Errors if:
///  - unable to connect to database; or
///  - if SQL query fails.
#[tracing::instrument(name = "Drafts query", skip(db_pool))]
pub async fn drafts_query(db_pool: &SqlitePool) -> Result<Vec<Post>, anyhow::Error> {
    let limit = 100;
    let rows = sqlx::query_as!(
        Post,
        r#"
SELECT
    "id",
    "title",
    "body",
    "published"
FROM
    "Post"
WHERE
    "published" = FALSE
LIMIT
    ?
         "#,
        limit
    )
    .fetch_all(db_pool)
    .await?;

    Ok(rows)
}

/// Returns a list of up to 100 published posts
///
/// # Errors
///
/// Errors if:
///  - unable to connect to database; or
///  - if SQL query fails.
#[tracing::instrument(name = "Posts query", skip(db_pool))]
pub async fn posts_query(db_pool: &SqlitePool) -> Result<Vec<Post>, anyhow::Error> {
    let limit = 100;
    let rows = sqlx::query_as!(
        Post,
        r#"
SELECT
    "id",
    "title",
    "body",
    "published"
FROM
    "Post"
WHERE
    "published" = TRUE
LIMIT
    ?
         "#,
        limit
    )
    .fetch_all(db_pool)
    .await?;

    Ok(rows)
}

/// Creates a new draft with `title` and `body`
/// Successful creation returns the created post
///
/// # Errors
///
/// Errors if:
///  - unable to connect to database; or
///  - if SQL query fails.
#[tracing::instrument(name = "Create draft mutation", skip(db_pool))]
pub async fn create_draft_mutation(
    db_pool: &SqlitePool,
    title: &str,
    body: &str,
) -> Result<Post, anyhow::Error> {
    let inserted_row = sqlx::query_as!(
        Post,
        r#"
INSERT INTO
    "Post" ("title", "body", "published")
VALUES
    ($1, $2, false)
RETURNING
    "id",
    "title",
    "body",
    "published"
"#,
        title,
        body
    )
    .fetch_one(db_pool)
    .await
    .inspect_err(|err| {
        tracing::error!("Failed to execute query: {err:?}");
    })
    .context("run create draft mutation for post")?;

    Ok(inserted_row)
}

/// Deletes draft matching `id`
/// Returns `DeleteDraftResponse` with error, if the query yields no post matching `id`
/// Successful deletion returns a `DeleteDraftResponse` with the deleted post
///
/// # Errors
///
/// Errors if:
///  - unable to connect to database; or
///  - if SQL query fails.
#[tracing::instrument(name = "Delete draft mutation", skip(db_pool))]
pub async fn delete_draft_mutation(
    db_pool: &SqlitePool,
    id: i64,
) -> Result<DeleteDraftResponse, anyhow::Error> {
    let deleted_row = sqlx::query_as!(
        Post,
        r#"
DELETE FROM
    "Post"
WHERE
    (
        "id" = $1
        AND "published" = FALSE
    )
RETURNING
    "id",
    "title",
    "body",
    "published"
     "#,
        id,
    )
    .fetch_optional(db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {e:?}");
        e
    })?;

    match deleted_row {
        Some(value) => Ok(DeleteDraftResponse::DeleteDraftSuccessResponse(
            DeleteDraftSuccessResponse { post: value },
        )),
        None => Ok(DeleteDraftResponse::DeleteDraftErrorResponse(
            DeleteDraftErrorResponse {
                error: UserInputError {
                    field: "id".to_string(),
                    message: format!("Did not find draft post with id `{id}`"),
                    received: id.to_string(),
                },
            },
        )),
    }
}

/// Publishes draft matching `id`
/// Returns `PublishResponse` with error, if the query yields no post matching `id`
/// Successful publishing returns a `PublishResponse` with the updated post
///
/// # Errors
///
/// Errors if:
///  - unable to connect to database; or
///  - if SQL query fails.
#[tracing::instrument(name = "Publish mutation", skip(db_pool))]
pub async fn publish_mutation(
    db_pool: &SqlitePool,
    id: i64,
) -> Result<PublishResponse, anyhow::Error> {
    let updated_row = sqlx::query_as!(
        Post,
        r#"
UPDATE
    "Post"
SET
    "published" = TRUE
WHERE
    ("id" = $1)
RETURNING
    "id",
    "title",
    "body",
    "published"
     "#,
        id,
    )
    .fetch_optional(db_pool)
    .await
    .inspect_err(|err| {
        tracing::error!("Failed to execute query: {err:?}");
    })?;

    match updated_row {
        Some(value) => Ok(PublishResponse::PublishSuccessResponse(
            PublishSuccessResponse { post: value },
        )),
        None => Ok(PublishResponse::PublishErrorResponse(
            PublishErrorResponse {
                error: UserInputError {
                    field: "id".to_string(),
                    message: format!("Did not find draft post with id `{id}`"),
                    received: id.to_string(),
                },
            },
        )),
    }
}
