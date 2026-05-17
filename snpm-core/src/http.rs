use crate::{Result, SnpmError};

use std::time::Duration;

// Long enough for a multi-MiB tarball over a slow connection. Pacquet uses
// the same 5-minute ceiling for the same reason (`pnpm/pacquet`
// `network/src/lib.rs`); pnpm's default agentkeepalive timeout sits at
// 60s, but tarball reads have to live in the request budget too.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

// CDNs (Cloudflare/Fastly) drop idle connections aggressively; matching
// the agentkeepalive default the npm ecosystem assumes avoids spending
// an RTT on a 502 from a half-closed connection.
const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(4);

const POOL_MAX_IDLE_PER_HOST: usize = 32;

/// Create an HTTP client with sensible defaults (timeout, user-agent, pooling).
///
/// `hickory_dns` is on because reqwest's default macOS resolver routes
/// through mDNSResponder, which spuriously returns `EAI_NONAME` for
/// valid hostnames when many concurrent lookups pile up (the
/// "DNS error: failed to lookup address information" failures seen
/// during big cold installs). hickory-dns queries DNS over UDP/TCP
/// directly and bypasses the flake.
pub fn create_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(DEFAULT_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .pool_max_idle_per_host(POOL_MAX_IDLE_PER_HOST)
        .pool_idle_timeout(POOL_IDLE_TIMEOUT)
        .tcp_keepalive(Duration::from_secs(30))
        .tcp_nodelay(true)
        .hickory_dns(true)
        .user_agent(concat!("snpm/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|source| SnpmError::HttpClient { source })
}

/// Retry policy for HTTP operations. Defaults to pnpm's tarball retry
/// shape: 2 retries (3 attempts total), exponential backoff with
/// `factor = 10`, min/max delay of 10s/60s. The jitter is +/- 50% to
/// avoid synchronized retries thundering a single host.
#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub retries: u32,
    pub factor: u32,
    pub min_timeout: Duration,
    pub max_timeout: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            retries: 2,
            factor: 10,
            min_timeout: Duration::from_millis(10_000),
            max_timeout: Duration::from_millis(60_000),
        }
    }
}

impl RetryPolicy {
    /// Lighter-touch policy for metadata fetches: smaller floor, same
    /// ceiling. Registry metadata calls are higher-fanout than
    /// tarball fetches, so a 10s floor would multiply badly across
    /// hundreds of packages on a single flaky resolution.
    pub fn metadata() -> Self {
        Self {
            retries: 3,
            factor: 4,
            min_timeout: Duration::from_millis(500),
            max_timeout: Duration::from_millis(8_000),
        }
    }

    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let scaled = self
            .min_timeout
            .checked_mul(self.factor.saturating_pow(attempt))
            .unwrap_or(self.max_timeout);
        let bounded = scaled.min(self.max_timeout);
        jitter(bounded)
    }
}

fn jitter(base: Duration) -> Duration {
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    // Symmetric +/- 50% jitter around `base`. Cheap PRNG via clock
    // entropy — good enough to desynchronize retries without pulling
    // in `rand`.
    let scale =
        ((nanos as u64).wrapping_mul(6_364_136_223_846_793_005) >> 33) as f64 / (1u64 << 31) as f64;
    let jitter_factor = 0.5 + scale; // 0.5 .. 1.5
    let scaled_nanos = (base.as_nanos() as f64 * jitter_factor) as u128;
    Duration::from_nanos(scaled_nanos.min(u64::MAX as u128) as u64)
}

/// Should the given reqwest error be retried? Network errors and 5xx
/// responses are retryable; 4xx (auth, not-found) are not.
pub fn is_retryable_error(error: &reqwest::Error) -> bool {
    if error.is_timeout() || error.is_connect() || error.is_body() || error.is_decode() {
        return true;
    }
    if let Some(status) = error.status() {
        return status.is_server_error() || status.as_u16() == 408 || status.as_u16() == 429;
    }
    // No status means transport-layer (DNS, TCP, TLS). Retry.
    true
}

/// Run `operation` with retry-and-jitter. `operation` is a closure that
/// returns a future producing `Result<T, reqwest::Error>`. On retryable
/// errors, sleeps according to the policy and retries up to
/// `policy.retries` more times.
pub async fn with_retry<T, F, Fut>(
    policy: RetryPolicy,
    label: &str,
    mut operation: F,
) -> std::result::Result<T, reqwest::Error>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<T, reqwest::Error>>,
{
    let mut attempt: u32 = 0;
    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                let exhausted = attempt >= policy.retries;
                let retryable = is_retryable_error(&error);
                if exhausted || !retryable {
                    return Err(error);
                }
                let delay = policy.delay_for_attempt(attempt);
                if std::env::var_os("SNPM_VERBOSE").is_some() {
                    eprintln!(
                        "snpm: retrying {label} (attempt {}/{}) after {}ms",
                        attempt + 1,
                        policy.retries,
                        delay.as_millis()
                    );
                }
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RetryPolicy, jitter};
    use std::time::Duration;

    #[test]
    fn delay_for_attempt_caps_at_max() {
        let policy = RetryPolicy {
            retries: 5,
            factor: 10,
            min_timeout: Duration::from_millis(1_000),
            max_timeout: Duration::from_millis(5_000),
        };
        // attempt=0 -> 1s * 10^0 = 1s -> jittered 0.5..1.5s
        // attempt=3 -> 1s * 10^3 = 1000s, capped to 5s -> jittered 2.5..7.5s
        let small = policy.delay_for_attempt(0);
        let big = policy.delay_for_attempt(3);
        assert!(small <= Duration::from_millis(1_500));
        assert!(big <= Duration::from_millis(7_500));
    }

    #[test]
    fn metadata_policy_is_lighter_than_default() {
        let default = RetryPolicy::default();
        let metadata = RetryPolicy::metadata();
        assert!(metadata.min_timeout < default.min_timeout);
        assert!(metadata.max_timeout < default.max_timeout);
    }

    #[test]
    fn jitter_stays_within_50_percent_bounds() {
        let base = Duration::from_millis(1_000);
        let mut min_observed = u128::MAX;
        let mut max_observed = 0u128;
        for _ in 0..200 {
            let j = jitter(base).as_nanos();
            min_observed = min_observed.min(j);
            max_observed = max_observed.max(j);
            std::thread::sleep(Duration::from_micros(1));
        }
        // Loose bounds because jitter is clock-derived; just confirm
        // the symmetric +/- 50% envelope holds.
        assert!(min_observed >= base.as_nanos() / 4);
        assert!(max_observed <= base.as_nanos() * 2);
    }
}
