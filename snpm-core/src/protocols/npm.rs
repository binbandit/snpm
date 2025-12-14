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
    protocol_name: &str,
) -> Result<RegistryPackage> {
    if let Some(cached) = crate::cache::load_metadata(config, name) {
        return Ok(cached);
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
        // Decide scheme: default registry uses configured scheme; others default to Bearer
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
