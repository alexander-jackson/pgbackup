use std::process::Stdio;

use color_eyre::eyre::Result;
use tokio::process::Command;
use tokio_postgres::{Client, NoTls};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

struct DatabaseConfig {
    user: String,
    password: Option<String>,
    database: String,
    host: String,
    port: u16,
}

impl From<&DatabaseConfig> for tokio_postgres::Config {
    fn from(value: &DatabaseConfig) -> Self {
        let mut config = tokio_postgres::Config::new();

        config
            .user(value.user.clone())
            .dbname(value.database.clone())
            .host(value.host.clone())
            .port(value.port);

        if let Some(password) = value.password.as_deref() {
            config.password(password.to_owned());
        }

        config
    }
}

#[tracing::instrument(skip(client))]
async fn discover_databases(client: &Client) -> Result<Vec<String>> {
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
async fn backup_database(config: &DatabaseConfig, database: &str) -> Result<()> {
    let mut command = Command::new("pg_dump");

    command
        .args(&[
            "-h",
            &config.host,
            "-p",
            &config.port.to_string(),
            "-d",
            database,
            "-U",
            &config.user,
        ])
        .stdout(Stdio::piped());

    if let Some(password) = config.password.as_deref() {
        command.env("PGPASSWORD", password);
    }

    let output = command.spawn()?.wait_with_output().await?;

    tracing::info!(bytes = %output.stdout.len(), "got some output from the dump");

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env()?,
        )
        .init();

    let config = DatabaseConfig {
        user: "alex".to_owned(),
        database: "postgres".to_owned(),
        host: "localhost".to_owned(),
        port: 5432,
        password: None,
    };

    let (client, connection) = tokio_postgres::Config::from(&config).connect(NoTls).await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    let databases = discover_databases(&client).await?;

    for database in databases {
        backup_database(&config, &database).await?;
    }

    Ok(())
}
