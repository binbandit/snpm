use crate::{Result, SnpmError};
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Create an HTTP client with sensible defaults (timeout, user-agent, pooling).
pub fn create_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(DEFAULT_TIMEOUT)
        .connect_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(32)
        .pool_idle_timeout(Duration::from_secs(60))
        .tcp_keepalive(Duration::from_secs(30))
        .tcp_nodelay(true)
        .user_agent(concat!("snpm/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|source| SnpmError::HttpClient { source })
}
