use anyhow::Context;
use async_graphql::SimpleObject;
use sqlx::SqlitePool;

#[derive(Debug, PartialEq, SimpleObject)]
pub struct Post {
    id: i64,
    title: String,
    body: String,
    published: bool,
}

#[derive(SimpleObject)]
/// Extra information on the input error
pub struct InputError {
    /// Field which had the invalid data
    field: String,

    /// Error description
    message: String,

    /// Input value for field
    received: String,
}

#[derive(SimpleObject)]
pub struct DeleteDraftResponse {
    /// Deleted post
    post: Option<Post>,

    /// Input error details
    error: Option<InputError>,
}

#[derive(SimpleObject)]
pub struct PublishResponse {
    /// Published post
    post: Option<Post>,

    /// Input error details
    error: Option<InputError>,
}

/// Return a list of up to 100 draft posts
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
    .fetch_optional(db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {e:?}");
        e
    })
    .context("run create draft mutation for post")?;

    Ok(inserted_row.expect("No new data inserted"))
}

/// Deletes draft matching `id`
/// Returns `DeleteDraftResponse` with error, if the query yields no post matching `id`
/// Successful deletion returns a `DeleteDraftResponse` with the deleted post
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
        Some(value) => Ok(DeleteDraftResponse {
            post: Some(value),
            error: None,
        }),
        None => Ok(DeleteDraftResponse {
            post: None,
            error: Some(InputError {
                field: "id".to_string(),
                message: format!("Did not find draft post with id `{id}`"),
                received: id.to_string(),
            }),
        }),
    }
}

/// Publishes draft matching `id`
/// Returns `PublishResponse` with error, if the query yields no post matching `id`
/// Successful publishing returns a `PublishResponse` with the updated post
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
    .map_err(|e| {
        tracing::error!("Failed to execute query: {e:?}");
        e
    })?;

    match updated_row {
        Some(value) => Ok(PublishResponse {
            post: Some(value),
            error: None,
        }),
        None => Ok(PublishResponse {
            post: None,
            error: Some(InputError {
                field: "id".to_string(),
                message: format!("Did not find draft post with id `{id}`"),
                received: id.to_string(),
            }),
        }),
    }
}

#[cfg(test)]
mod tests {
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use crate::{
        database::run_migrations,
        model::post::{create_draft_mutation, publish_mutation},
    };

    use super::{posts_query, Post};

    /// Generates fresh in-memory `SQLite` database and runs migrations.  Can be called from each
    /// test.
    async fn get_db_pool() -> SqlitePool {
        let database_url = "sqlite://:memory:";

        let db_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(database_url)
            .await
            .unwrap();

        run_migrations(&db_pool).await;

        db_pool
    }

    #[tokio::test]
    async fn posts_query_returns_expected_output_with_no_posts() {
        // arrange
        let db_pool = get_db_pool().await;

        // act
        let result = posts_query(&db_pool).await.unwrap();

        // assert
        assert_eq!(result, Vec::<Post>::new());
    }

    #[tokio::test]
    async fn posts_query_returns_expected_output_with_posts() {
        // arrange
        let db_pool = get_db_pool().await;
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
}
