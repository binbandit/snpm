use crate::project::BinField;
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use super::super::bundled::BundledDependencies;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PeerDependencyMeta {
    #[serde(default)]
    pub optional: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    #[serde(default, rename = "bundledDependencies")]
    pub bundled_dependencies: Option<BundledDependencies>,
    #[serde(default, rename = "bundleDependencies")]
    pub bundle_dependencies: Option<BundledDependencies>,
    pub dist: RegistryDist,
    #[serde(default)]
    pub os: Vec<String>,
    #[serde(default)]
    pub cpu: Vec<String>,
    #[serde(default)]
    pub bin: Option<serde_json::Value>,
}

impl RegistryVersion {
    pub fn get_bundled_dependencies(&self) -> Option<&BundledDependencies> {
        self.bundled_dependencies
            .as_ref()
            .or(self.bundle_dependencies.as_ref())
    }

    pub fn has_bin(&self) -> bool {
        self.bin
            .as_ref()
            .map(|value| !value.is_null())
            .unwrap_or(false)
    }

    pub fn bin_definition(&self) -> Option<BinField> {
        self.bin
            .as_ref()
            .and_then(|value| serde_json::from_value::<BinField>(value.clone()).ok())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegistryDist {
    pub tarball: String,
    #[serde(default)]
    pub integrity: Option<String>,
}
