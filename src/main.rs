use std::time::Duration;

use aws_config::BehaviorVersion;
use aws_sdk_s3::primitives::ByteStream;
use chrono::Utc;
use color_eyre::eyre::Result;
use tokio_postgres::NoTls;
use tracing::level_filters::LevelFilter;
use tracing_error::ErrorLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

mod config;
mod databases;
mod utils;

use crate::config::DatabaseConfig;
use crate::databases::{discover, dump};
use crate::utils::{compress, get_env_var};

fn setup() -> Result<()> {
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

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    setup()?;

    let sdk_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let s3_client = aws_sdk_s3::Client::new(&sdk_config);
    let bucket = get_env_var("S3_BUCKET")?;

    loop {
        let now = Utc::now();
        let date = now.format("%Y-%m-%d");

        let span = tracing::info_span!("main", %date);
        let _guard = span.enter();

        let config = DatabaseConfig::from_env()?;
        let (client, connection) = tokio_postgres::Config::from(&config).connect(NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        let databases = discover(&client).await?;

        for database in databases {
            let dump = dump(&config, &database).await?;
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

        tokio::time::sleep(Duration::from_secs(60 * 60 * 24)).await;
    }
}
