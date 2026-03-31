mod environment;
mod execute;

use crate::Result;
use crate::config::OfflineMode;

use environment::prepare_dlx_environment;
use execute::run_bin;

/// Run a package binary with default online mode.
pub async fn dlx(
    config: &crate::SnpmConfig,
    package_spec: String,
    arguments: Vec<String>,
) -> Result<()> {
    dlx_with_offline(config, package_spec, arguments, OfflineMode::Online).await
}

/// Run a package binary respecting offline mode.
pub async fn dlx_with_offline(
    config: &crate::SnpmConfig,
    package_spec: String,
    arguments: Vec<String>,
    offline_mode: OfflineMode,
) -> Result<()> {
    let environment = prepare_dlx_environment(config, &package_spec, offline_mode).await?;
    run_bin(&environment.bin_path, arguments)
}
