use std::io::Write;

use color_eyre::eyre::{eyre, Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;

#[track_caller]
pub fn get_env_var(key: &str) -> Result<String> {
    std::env::var(key).wrap_err_with(|| eyre!("failed to get environment variable with key {key}"))
}

#[tracing::instrument(skip(content))]
pub fn compress(content: &[u8]) -> Result<Vec<u8>> {
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
