use crate::config::OfflineMode;
use crate::console;
use crate::registry::RegistryPackage;
use crate::{Result, SnpmConfig, SnpmError};
use reqwest::Client;
use reqwest::header::{ACCEPT, HeaderValue};
use std::env;
use std::time::Instant;

pub async fn fetch_package(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
) -> Result<RegistryPackage> {
    fetch_package_with_offline(config, client, name, OfflineMode::Online).await
}

pub async fn fetch_package_with_offline(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
    offline_mode: OfflineMode,
) -> Result<RegistryPackage> {
    let compat = jsr_compat_name(name);

    if let Some(cached) = crate::cache::load_metadata_with_offline(config, &compat, offline_mode) {
        return Ok(cached);
    }

    // In Offline mode, if cache miss, we fail
    if matches!(offline_mode, OfflineMode::Offline) {
        return Err(SnpmError::OfflineRequired {
            resource: format!("jsr package metadata for {}", name),
        });
    }

    let encoded = super::encode_package_name(&compat);
    let base = jsr_registry_base();
    let url = format!("{}/{}", base.trim_end_matches('/'), encoded);

    let mut request = client.get(&url);

    request = request.header(
        ACCEPT,
        HeaderValue::from_static(
            "application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8, */*",
        ),
    );

    let mut auth_token = config.auth_token_for_url(&url).map(|t| t.to_string());

    if auth_token.is_none()
        && config.always_auth
        && let Some(default_host) = crate::config::host_from_url(&config.default_registry)
        && let Some(req_host) = crate::config::host_from_url(&url)
        && req_host == default_host
        && let Some(def_tok) = config.default_registry_auth_token.as_ref()
    {
        auth_token = Some(def_tok.clone());
    }

    if let Some(token) = auth_token {
        let mut use_basic = false;
        if let Some(default_host) = crate::config::host_from_url(&config.default_registry)
            && let Some(req_host) = crate::config::host_from_url(&url)
            && req_host == default_host
        {
            use_basic = matches!(
                config.default_registry_auth_scheme,
                crate::config::AuthScheme::Basic
            );
        }

        let header_value = if use_basic {
            format!("Basic {}", token)
        } else {
            format!("Bearer {}", token)
        };
        request = request.header("authorization", header_value);
    }

    if console::is_logging_enabled() {
        console::verbose(&format!(
            "registry request (jsr): name={} compat={} url={}",
            name, compat, url
        ));
    }
    let started = Instant::now();

    let response = request.send().await.map_err(|source| SnpmError::Http {
        url: url.clone(),
        source,
    })?;

    let status = response.status();
    if console::is_logging_enabled() {
        console::verbose(&format!(
            "registry response (jsr): name={} status={} in {:.3}s",
            name,
            status.as_u16(),
            started.elapsed().as_secs_f64()
        ));
    }

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

    if console::is_logging_enabled() {
        console::verbose(&format!(
            "registry decode (jsr): name={} versions={} dist_tags={}",
            name,
            package.versions.len(),
            package.dist_tags.len()
        ));
    }

    let _ = crate::cache::save_metadata(config, &compat, &package);

    Ok(package)
}

fn jsr_registry_base() -> String {
    if let Ok(value) = env::var("SNPM_REGISTRY_JSR") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_string();
        }
    }

    "https://npm.jsr.io".to_string()
}

fn jsr_compat_name(name: &str) -> String {
    if name.starts_with("@jsr/") {
        return name.to_string();
    }

    if let Some(stripped) = name.strip_prefix('@') {
        if let Some((scope, pkg)) = stripped.split_once('/') {
            return format!("@jsr/{}__{}", scope, pkg);
        } else {
            return format!("@jsr/{}", stripped);
        }
    }

    if let Some((scope, pkg)) = name.split_once('/') {
        return format!("@jsr/{}__{}", scope, pkg);
    }

    format!("@jsr/{}", name)
}
