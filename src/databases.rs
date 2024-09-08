use std::process::Stdio;

use color_eyre::eyre::{eyre, Context, Result};
use tokio::process::Command;
use tokio_postgres::Client;

use crate::config::DatabaseConfig;

#[tracing::instrument(skip(client))]
pub async fn discover(client: &Client) -> Result<Vec<String>> {
    let query = r#"
        SELECT datname
        FROM pg_database
        WHERE datname NOT IN (
            'postgres',
            'template0',
            'template1'
        )
    "#;

    let rows = client.query(query, &[]).await?;
    let databases = rows.into_iter().map(|row| row.get(0)).collect();

    tracing::info!(?databases, "discovered some targets for backup");

    Ok(databases)
}

#[tracing::instrument(skip(config))]
pub async fn dump(config: &DatabaseConfig, database: &str) -> Result<Vec<u8>> {
    let mut command = Command::new("pg_dump");

    command
        .args([
            "-h",
            &config.host,
            "-p",
            &config.port.to_string(),
            "-d",
            database,
            "-U",
            &config.username,
        ])
        .stdout(Stdio::piped());

    if let Some(password) = config.password.as_deref() {
        command.env("PGPASSWORD", password);
    }

    let stdout = command
        .spawn()
        .wrap_err_with(|| eyre!("failed to run `pg_dump` command"))?
        .wait_with_output()
        .await?
        .stdout;

    tracing::info!(bytes = %stdout.len(), "got some output from the dump");

    Ok(stdout)
}
