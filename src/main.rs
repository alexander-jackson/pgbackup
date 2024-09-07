use color_eyre::eyre::Result;
use tokio_postgres::{Client, NoTls};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

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

    Ok(databases)
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

    let (client, connection) = tokio_postgres::Config::new()
        .user("alex")
        .dbname("postgres")
        .host("localhost")
        .port(5432)
        .connect(NoTls)
        .await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    let databases = discover_databases(&client).await?;

    tracing::info!(?databases, "discovered the databases to backup");

    Ok(())
}
