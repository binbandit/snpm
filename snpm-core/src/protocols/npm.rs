use crate::config::OfflineMode;
use crate::console;
use crate::registry::RegistryPackage;
use crate::{Result, SnpmConfig, SnpmError};
use reqwest::Client;
use reqwest::header::{ACCEPT, HeaderValue};
use std::env;
use std::time::Instant;

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
    // Try to load from cache first
    if let Some(cached) = crate::cache::load_metadata_with_offline(config, name, offline_mode) {
        return Ok(cached);
    }

    // In Offline mode, if cache miss, we fail
    if matches!(offline_mode, OfflineMode::Offline) {
        return Err(SnpmError::OfflineRequired {
            resource: format!("package metadata for {}", name),
        });
    }

    let encoded = super::encode_package_name(name);
    let base = npm_like_registry_for_package(config, protocol_name, name);
    let url = format!("{}/{}", base.trim_end_matches('/'), encoded);

    let mut request = client.get(&url);

    request = request.header(
        ACCEPT,
        HeaderValue::from_static(
            "application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8, */*",
        ),
    );

    if let Some(header_value) = config.authorization_header_for_url(&url) {
        request = request.header("authorization", header_value);
    }

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

    let package = response
        .error_for_status()
        .map_err(|source| SnpmError::Http {
            url: url.clone(),
            source,
        })?
        .json::<RegistryPackage>()
        .await
        .map_err(|source| SnpmError::Http {
            url: url.clone(),
            source,
        })?;

    console::verbose(&format!(
        "registry decode: name={} versions={} dist_tags={}",
        name,
        package.versions.len(),
        package.dist_tags.len()
    ));

    let _ = crate::cache::save_metadata(config, name, &package);

    Ok(package)
}

fn npm_like_registry_for_package(config: &SnpmConfig, protocol_name: &str, name: &str) -> String {
    if protocol_name == "npm" {
        return npm_registry_for_package(config, name);
    }

    let key = format!("SNPM_REGISTRY_{}", protocol_name.to_uppercase());
    if let Ok(value) = env::var(&key) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_string();
        }
    }

    config.default_registry.clone()
}

fn npm_registry_for_package(config: &SnpmConfig, name: &str) -> String {
    if let Some((scope, _)) = name.split_once('/')
        && scope.starts_with('@')
        && let Some(reg) = config.scoped_registries.get(scope)
    {
        return reg.clone();
    }

    config.default_registry.clone()
}
