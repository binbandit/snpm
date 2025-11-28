use crate::{Result, SnpmConfig, SnpmError};
use reqwest::Client;
use reqwest::header::{ACCEPT, HeaderValue};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;

#[derive(Clone, Debug, Deserialize)]
pub struct RegistryPackage {
    pub versions: BTreeMap<String, RegistryVersion>,
    #[serde(default)]
    pub time: BTreeMap<String, String>,
    #[serde(default, rename = "dist-tags")]
    pub dist_tags: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PeerDependencyMeta {
    #[serde(default)]
    pub optional: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RegistryVersion {
    pub version: String,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    #[serde(default, rename = "optionalDependencies")]
    pub optional_dependencies: BTreeMap<String, String>,
    #[serde(default, rename = "peerDependencies")]
    pub peer_dependencies: BTreeMap<String, String>,
    #[serde(default, rename = "peerDependenciesMeta")]
    pub peer_dependencies_meta: BTreeMap<String, PeerDependencyMeta>,
    pub dist: RegistryDist,
    #[serde(default)]
    pub os: Vec<String>,
    #[serde(default)]
    pub cpu: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct RegistryProtocol {
    pub name: String,
}

impl RegistryProtocol {
    pub fn npm() -> Self {
        RegistryProtocol {
            name: "npm".to_string(),
        }
    }

    pub fn jsr() -> Self {
        RegistryProtocol {
            name: "jsr".to_string(),
        }
    }

    pub fn custom(name: &str) -> Self {
        RegistryProtocol {
            name: name.to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct RegistryDist {
    pub tarball: String,
    #[serde(default)]
    pub integrity: Option<String>,
}

pub async fn fetch_package(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
    protocol: &RegistryProtocol,
) -> Result<RegistryPackage> {
    if protocol.name == "jsr" {
        fetch_jsr_package(config, client, name).await
    } else {
        fetch_npm_like_package(config, client, name, &protocol.name).await
    }
}

fn encode_package_name(name: &str) -> String {
    if name.starts_with('@') {
        name.replace('/', "%2F")
    } else {
        name.to_string()
    }
}

async fn fetch_npm_like_package(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
    protocol_name: &str,
) -> Result<RegistryPackage> {
    let encoded = encode_package_name(name);
    let base = npm_like_registry_for_package(config, protocol_name, name);
    let url = format!("{}/{}", base.trim_end_matches('/'), encoded);

    let mut request = client.get(&url);

    request = request.header(
        ACCEPT,
        HeaderValue::from_static(
            "application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8, */*",
        ),
    );

    if let Some(token) = config.auth_token_for_url(&url) {
        let header_value = format!("Bearer {}", token);
        request = request.header("authorization", header_value);
    }

    let response = request.send().await.map_err(|source| SnpmError::Http {
        url: url.clone(),
        source,
    })?;

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
    if let Some((scope, _)) = name.split_once('/') {
        if scope.starts_with('@') {
            if let Some(reg) = config.scoped_registries.get(scope) {
                return reg.clone();
            }
        }
    }

    config.default_registry.clone()
}

async fn fetch_jsr_package(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
) -> Result<RegistryPackage> {
    let compat = jsr_compat_name(name);
    let encoded = encode_package_name(&compat);
    let base = jsr_registry_base();
    let url = format!("{}/{}", base.trim_end_matches('/'), encoded);

    let mut request = client.get(&url);

    request = request.header(
        ACCEPT,
        HeaderValue::from_static(
            "application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8, */*",
        ),
    );

    if let Some(token) = config.auth_token_for_url(&url) {
        let header_value = format!("Bearer {}", token);
        request = request.header("authorization", header_value);
    }

    let response = request.send().await.map_err(|source| SnpmError::Http {
        url: url.clone(),
        source,
    })?;

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
