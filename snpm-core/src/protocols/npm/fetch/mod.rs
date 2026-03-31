mod request;
mod response;

use crate::config::OfflineMode;
use crate::console;
use crate::registry::RegistryPackage;
use crate::{Result, SnpmConfig, SnpmError};

use reqwest::Client;
use std::time::Instant;

use request::{build_request, registry_url};
use response::handle_registry_response;

pub(super) async fn fetch_registry_package(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
    protocol_name: &str,
    offline_mode: OfflineMode,
) -> Result<RegistryPackage> {
    if let Some(cached) = response::load_cached_package(config, name, offline_mode) {
        return Ok(cached);
    }

    if matches!(offline_mode, OfflineMode::Offline) {
        return Err(SnpmError::OfflineRequired {
            resource: format!("package metadata for {}", name),
        });
    }

    let url = registry_url(config, protocol_name, name);
    let request = build_request(config, client, name, &url);

    console::verbose(&format!(
        "registry request: name={} protocol={} url={}",
        name, protocol_name, url
    ));
    let started = Instant::now();

    let response = request.send().await.map_err(|source| SnpmError::Http {
        url: url.clone(),
        source,
    })?;

    let status = response.status();
    console::verbose(&format!(
        "registry response: name={} status={} in {:.3}s",
        name,
        status.as_u16(),
        started.elapsed().as_secs_f64()
    ));

    handle_registry_response(config, name, &url, response).await
}
