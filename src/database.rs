use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    Sqlite, SqlitePool,
};

/// .
///
/// # Panics
///
/// Panics if unbale to create the database.
pub async fn create(db_url: &str) {
    if Sqlite::database_exists(db_url).await.unwrap_or(false) {
        tracing::info!("Database already exists");
    } else {
        tracing::info!("Creating database");
        match Sqlite::create_database(db_url).await {
            Ok(()) => tracing::info!("Database created successfully"),
            Err(error) => panic!("Error creating database: {error}"),
        }
    }
}

/// .
///
/// # Panics
///
/// Panics if unable to run migrations successfully.
pub async fn run_migrations(db_pool: &SqlitePool) {
    let migrations = std::path::PathBuf::from("migrations");

    let results = Migrator::new(migrations).await.unwrap().run(db_pool).await;

    match results {
        Ok(()) => tracing::info!("Migrations run successfully"),
        Err(error) => panic!("Error running migrations: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use assert_fs::fixture::PathChild;
    use sqlx::SqlitePool;

    use crate::database::{create, run_migrations};

    #[tokio::test]
    async fn create_does_not_panic_if_database_already_exists() {
        // arrange
        let temp_dir = assert_fs::TempDir::new().unwrap();
        temp_dir.child("sqlite.db");

        let database_url = format!("sqlite://{}", temp_dir.join("sqlite.db").to_str().unwrap());
        create(&database_url).await;

        // act
        create(&database_url).await;

        // assert
    }

    #[tokio::test]
    async fn create_creates_database_file() {
        // arrange
        let temp_dir = assert_fs::TempDir::new().unwrap();
        let database_file = temp_dir.child("sqlite.db");

        let database_url = format!("sqlite://{}", database_file.path().to_str().unwrap());

        // act
        create(&database_url).await;

        // assert
        assert!(database_file.is_file());

        let db_file_size = database_file.metadata().unwrap().len();
        assert!(db_file_size > 0);
    }

    #[tokio::test]
    async fn run_migrations_creates_tables() {
        // arrange
        let database_url = "sqlite://:memory:";
        let db_pool = SqlitePool::connect(database_url)
            .await
            .expect("SQLite database should be reachable");

        // act
        run_migrations(&db_pool).await;

        // assert
        let query_outcome = sqlx::query_unchecked!(
            r#"
SELECT
    name
FROM
    sqlite_schema
WHERE
    TYPE = 'table'
    AND name NOT LIKE 'sqlite_%';
"#
        )
        .fetch_all(&db_pool)
        .await
        .unwrap();

        insta::assert_snapshot!(format!("{query_outcome:?}"));
    }
}
