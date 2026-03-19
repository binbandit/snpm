use crate::{Result, SnpmError};
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Create an HTTP client with sensible defaults (timeout, user-agent).
pub fn create_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(DEFAULT_TIMEOUT)
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|source| SnpmError::HttpClient { source })
}
