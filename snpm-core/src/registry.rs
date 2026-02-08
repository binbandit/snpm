pub mod types;

pub use types::*;

use crate::config::OfflineMode;
use crate::{Result, SnpmConfig};
use reqwest::Client;

pub async fn fetch_package(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
    protocol: &RegistryProtocol,
) -> Result<RegistryPackage> {
    fetch_package_with_offline(config, client, name, protocol, OfflineMode::Online).await
}

pub async fn fetch_package_with_offline(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
    protocol: &RegistryProtocol,
    offline_mode: OfflineMode,
) -> Result<RegistryPackage> {
    if protocol.name == "jsr" {
        // jsr uses npm protocol under the hood
        crate::protocols::jsr::fetch_package_with_offline(config, client, name, offline_mode).await
    } else if protocol.name == "file" {
        // file protocol is always local, no network needed
        crate::protocols::file::fetch_package(config, name).await
    } else if protocol.name == "git" {
        // git protocol requires network unless already cloned
        crate::protocols::git::fetch_package(config, name).await
    } else {
        crate::protocols::npm::fetch_package_with_offline(
            config,
            client,
            name,
            &protocol.name,
            offline_mode,
        )
        .await
    }
}
