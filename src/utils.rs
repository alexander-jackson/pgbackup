use std::cmp::Ordering;
use std::io::Write;
use std::time::Duration;

use chrono::NaiveTime;
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

pub fn get_initial_offset(now: NaiveTime, schedule_time: NaiveTime) -> Duration {
    match now.cmp(&schedule_time) {
        Ordering::Equal => Duration::from_secs(0),
        Ordering::Less => {
            let secs = (schedule_time - now).num_seconds();
            Duration::from_secs(secs as u64)
        }
        Ordering::Greater => {
            let secs = 86400 - (now - schedule_time).num_seconds();
            Duration::from_secs(secs as u64)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::NaiveTime;

    use crate::utils::get_initial_offset;

    #[test]
    fn computes_no_offset_when_running_at_schedule_time() {
        let now = NaiveTime::from_hms_opt(22, 30, 0).unwrap();
        let schedule_time = now;

        let offset = get_initial_offset(now, schedule_time);

        assert_eq!(offset, Duration::from_secs(0));
    }

    #[test]
    fn computes_correct_offset_when_running_before_schedule_time() {
        let now = NaiveTime::from_hms_opt(22, 0, 0).unwrap();
        let schedule_time = NaiveTime::from_hms_opt(22, 30, 0).unwrap();

        let offset = get_initial_offset(now, schedule_time);

        assert_eq!(offset, Duration::from_secs(30 * 60));
    }

    #[test]
    fn computes_correct_offset_when_running_after_schedule_time() {
        let now = NaiveTime::from_hms_opt(22, 45, 0).unwrap();
        let schedule_time = NaiveTime::from_hms_opt(22, 30, 0).unwrap();

        let offset = get_initial_offset(now, schedule_time);

        assert_eq!(offset, Duration::from_secs(23 * 60 * 60 + 45 * 60));
    }
}
