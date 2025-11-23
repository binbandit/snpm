use crate::{Result, SnpmConfig, SnpmError};
use serde::Deserialize;
use std::collections::BTreeMap;

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
pub enum RegistryProtocol {
    Npm,
    Jsr,
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
    protocol: RegistryProtocol,
) -> Result<RegistryPackage> {
    match protocol {
        RegistryProtocol::Npm => fetch_npm_package(config, name).await,
        RegistryProtocol::Jsr => fetch_jsr_package(name).await,
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

async fn fetch_npm_package(config: &SnpmConfig, name: &str) -> Result<RegistryPackage> {
    let encoded = encode_package_name(name);
    let base = npm_registry_for_package(config, name);
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
    // jsr uses a different API; we assume a simple metadata endpoint here
    let encoded = encode_package_name(name);
    let url = format!("https://npm.jsr.io/{}", encoded);

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
