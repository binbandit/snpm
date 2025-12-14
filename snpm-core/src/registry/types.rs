use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegistryPackage {
    pub versions: BTreeMap<String, RegistryVersion>,
    #[serde(default)]
    pub time: BTreeMap<String, serde_json::Value>,
    #[serde(default, rename = "dist-tags")]
    pub dist_tags: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PeerDependencyMeta {
    #[serde(default)]
    pub optional: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BundledDependencies {
    List(Vec<String>),
    All(bool),
}

impl BundledDependencies {
    pub fn to_set(&self, all_deps: &BTreeMap<String, String>) -> BTreeSet<String> {
        match self {
            BundledDependencies::List(list) => list.iter().cloned().collect(),
            BundledDependencies::All(true) => all_deps.keys().cloned().collect(),
            BundledDependencies::All(false) => BTreeSet::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            BundledDependencies::List(list) => list.is_empty(),
            BundledDependencies::All(val) => !val,
        }
    }
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
        self.bin.as_ref().map(|b| !b.is_null()).unwrap_or(false)
    }
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegistryDist {
    pub tarball: String,
    #[serde(default)]
    pub integrity: Option<String>,
}
