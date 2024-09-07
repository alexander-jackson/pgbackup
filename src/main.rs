use std::io::Write;
use std::process::Stdio;

use aws_config::BehaviorVersion;
use aws_sdk_s3::primitives::ByteStream;
use chrono::Utc;
use color_eyre::eyre::{eyre, Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use tokio::process::Command;
use tokio_postgres::{Client, NoTls};
use tracing::level_filters::LevelFilter;
use tracing_error::ErrorLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[track_caller]
fn get_env_var(key: &str) -> Result<String> {
    std::env::var(key).wrap_err_with(|| eyre!("failed to get environment variable with key {key}"))
}

struct DatabaseConfig {
    username: String,
    password: Option<String>,
    database: String,
    host: String,
    port: u16,
}

impl DatabaseConfig {
    fn from_env() -> Result<Self> {
        let username = get_env_var("USERNAME")?;
        let password = get_env_var("PASSWORD").ok();
        let database = get_env_var("ROOT_DATABASE")?;
        let host = get_env_var("DATABASE_HOST")?;
        let port = get_env_var("DATABASE_PORT")?.parse()?;

        Ok(Self {
            username,
            password,
            database,
            host,
            port,
        })
    }
}

impl From<&DatabaseConfig> for tokio_postgres::Config {
    fn from(value: &DatabaseConfig) -> Self {
        let mut config = tokio_postgres::Config::new();

        config
            .user(value.username.clone())
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
async fn get_dump_for_database(config: &DatabaseConfig, database: &str) -> Result<Vec<u8>> {
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
            &config.username,
        ])
        .stdout(Stdio::piped());

    if let Some(password) = config.password.as_deref() {
        command.env("PGPASSWORD", password);
    }

    let output = command.spawn()?.wait_with_output().await?;
    let stdout = output.stdout;

    tracing::info!(bytes = %stdout.len(), "got some output from the dump");

    Ok(stdout)
}

#[tracing::instrument(skip(content))]
fn compress(content: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(content)?;
    let compressed = encoder.finish()?;

    tracing::info!(
        input_size = %content.len(),
        output_size = %compressed.len(),
        "compressed some data using gzip"
    );

    Ok(compressed)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let fmt_layer = tracing_subscriber::fmt::layer();
    let error_layer = ErrorLayer::default();
    let env_filter_layer = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()?;

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(error_layer)
        .with(env_filter_layer)
        .init();

    let now = Utc::now();
    let date = now.format("%Y-%m-%d");

    let span = tracing::info_span!("main", %date);
    let _guard = span.enter();

    let sdk_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let s3_client = aws_sdk_s3::Client::new(&sdk_config);
    let bucket = get_env_var("S3_BUCKET")?;

    let config = DatabaseConfig::from_env()?;
    let (client, connection) = tokio_postgres::Config::from(&config).connect(NoTls).await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    let databases = discover_databases(&client).await?;

    for database in databases {
        if database != "tasks" {
            continue;
        }

        let dump = get_dump_for_database(&config, &database).await?;
        let compressed = compress(&dump)?;

        let key = format!("{database}/{database}.{date}.sql.gz");

        s3_client
            .put_object()
            .bucket(&bucket)
            .key(&key)
            .body(ByteStream::from(compressed))
            .send()
            .await?;

        tracing::info!(%bucket, %key, "persisted a backup to S3");
    }

    Ok(())
}
