pub mod types;

pub use types::*;

use crate::{Result, SnpmConfig};
use reqwest::Client;

pub async fn fetch_package(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
    protocol: &RegistryProtocol,
) -> Result<RegistryPackage> {
    if protocol.name == "jsr" {
        crate::protocols::jsr::fetch_package(config, client, name).await
    } else if protocol.name == "file" {
        crate::protocols::file::fetch_package(config, name).await
    } else if protocol.name == "git" {
        crate::protocols::git::fetch_package(config, name).await
    } else {
        crate::protocols::npm::fetch_package(config, client, name, &protocol.name).await
    }
}
