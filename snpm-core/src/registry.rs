use crate::{Result, SnpmError};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct RegistryPackage {
    pub versions: BTreeMap<String, RegistryVersion>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RegistryVersion {
    pub version: String,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    pub dist: RegistryDist,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RegistryDist {
    pub tarball: String,
    #[serde(default)]
    pub integrity: Option<String>,
}

pub async fn fetch_package(name: &str) -> Result<RegistryPackage> {
    let encoded = encode_package_name(name);
    let url = format!("https://registry.npmjs.org/{}", encoded);
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

fn encode_package_name(name: &str) -> String {
    if name.starts_with('@') {
        name.replace('/', "%2F")
    } else {
        name.to_string()
    }
}
