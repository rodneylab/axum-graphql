#![warn(clippy::all, clippy::pedantic)]

mod database;
mod model;
mod observability;
mod routes;
mod startup;

use std::env;

use dotenvy::dotenv;

use crate::{
    database::create as create_database,
    observability::{
        metrics::create_prometheus_recorder, tracing::create_tracing_subscriber_from_env,
    },
    startup::Application,
};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    create_tracing_subscriber_from_env();

    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://sqlite.db".into());
    create_database(&database_url).await;

    let recorder_handle = create_prometheus_recorder();

    let application = Application::build(&database_url, recorder_handle).await?;
    application.run_until_stopped().await?;

    Ok(())
}
