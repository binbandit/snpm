use crate::{Result, SnpmConfig, SnpmError};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;

#[derive(Clone, Debug, Deserialize)]
pub struct RegistryPackage {
    pub versions: BTreeMap<String, RegistryVersion>,
    #[serde(default)]
    pub time: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RegistryVersion {
    pub version: String,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    #[serde(default, rename = "optionalDependencies")]
    pub optional_dependencies: BTreeMap<String, String>,
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
struct JsrPackage {
    #[serde(default)]
    pub versions: BTreeMap<String, JsrVersion>,
}

#[derive(Clone, Debug, Deserialize)]
struct JsrVersion {
    pub version: String,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    #[serde(default)]
    pub optional_dependencies: BTreeMap<String, String>,
    pub dist: JsrDist,
    #[serde(default)]
    pub os: Vec<String>,
    #[serde(default)]
    pub cpu: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct JsrDist {
    pub tarball: String,
    #[serde(default)]
    pub integrity: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RegistryDist {
    pub tarball: String,
    #[serde(default)]
    pub integrity: Option<String>,
}

pub async fn fetch_package(
    config: &SnpmConfig,
    name: &str,
    protocol: &RegistryProtocol,
) -> Result<RegistryPackage> {
    if protocol.name == "jsr" {
        fetch_jsr_package(name).await
    } else {
        fetch_npm_like_package(config, name, &protocol.name).await
    }
}

fn encode_package_name(name: &str) -> String {
    if name.starts_with('@') {
        name.replace('/', "%2F")
    } else {
        name.to_string()
    }
}

fn jsr_to_registry(pkg: JsrPackage) -> RegistryPackage {
    let mut versions = BTreeMap::new();

    for (ver, jsr_ver) in pkg.versions.into_iter() {
        let v = RegistryVersion {
            version: jsr_ver.version,
            dependencies: jsr_ver.dependencies,
            optional_dependencies: jsr_ver.optional_dependencies,
            dist: RegistryDist {
                tarball: jsr_ver.dist.tarball,
                integrity: jsr_ver.dist.integrity,
            },
            os: jsr_ver.os,
            cpu: jsr_ver.cpu,
        };
        versions.insert(ver, v);
    }

    RegistryPackage {
        versions,
        time: BTreeMap::new(),
    }
}

async fn fetch_npm_like_package(
    config: &SnpmConfig,
    name: &str,
    protocol_name: &str,
) -> Result<RegistryPackage> {
    let encoded = encode_package_name(name);
    let base = npm_like_registry_for_package(config, protocol_name, name);
    let url = format!("{}/{}", base.trim_end_matches('/'), encoded);

    let response = reqwest::get(&url).await.map_err(|source| SnpmError::Http {
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

async fn fetch_jsr_package(name: &str) -> Result<RegistryPackage> {
    let encoded = encode_package_name(name);
    let base = jsr_registry_base();
    let url = format!("{}/{}", base.trim_end_matches('/'), encoded);

    let response = reqwest::get(&url).await.map_err(|source| SnpmError::Http {
        url: url.clone(),
        source,
    })?;

    let pkg = response
        .error_for_status()
        .map_err(|source| SnpmError::Http {
            url: url.clone(),
            source,
        })?
        .json::<JsrPackage>()
        .await
        .map_err(|source| SnpmError::Http {
            url: url.clone(),
            source,
        })?;

    Ok(jsr_to_registry(pkg))
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
