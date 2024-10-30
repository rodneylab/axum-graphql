use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    Sqlite, SqlitePool,
};

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

pub async fn run_migrations(db_pool: &SqlitePool) {
    let migrations = std::path::PathBuf::from("migrations");

    let results = Migrator::new(migrations).await.unwrap().run(db_pool).await;

    match results {
        Ok(()) => tracing::info!("Migrations run successfully"),
        Err(error) => panic!("Error running migrations: {error}"),
    }
}
