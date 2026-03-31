mod fetch;
mod registry;

use crate::config::OfflineMode;
use crate::registry::RegistryPackage;
use crate::{Result, SnpmConfig};
use reqwest::Client;

use fetch::fetch_registry_package;

/// Fetch package metadata (Online mode).
pub async fn fetch_package(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
    protocol_name: &str,
) -> Result<RegistryPackage> {
    fetch_package_with_offline(config, client, name, protocol_name, OfflineMode::Online).await
}

/// Fetch package metadata respecting offline mode.
pub async fn fetch_package_with_offline(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
    protocol_name: &str,
    offline_mode: OfflineMode,
) -> Result<RegistryPackage> {
    fetch_registry_package(config, client, name, protocol_name, offline_mode).await
}
