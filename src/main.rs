#![warn(clippy::all, clippy::pedantic)]

use std::env;

use axum_graphql::{
    database::create as create_database, observability::initialise_observability,
    startup::Application,
};
use dotenvy::dotenv;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let otel_providers = initialise_observability();

    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://sqlite.db".into());
    create_database(&database_url).await;

    let application = Application::build(&database_url, ("127.0.0.1", 8000)).await?;
    application.run_until_stopped(otel_providers).await?;

    Ok(())
}
