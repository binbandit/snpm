pub mod file;
pub mod git;
pub mod jsr;
pub mod npm;

use crate::project::Manifest;
use crate::registry::{RegistryDist, RegistryVersion};
use std::collections::BTreeMap;

pub fn registry_version_from_manifest(manifest: Manifest, dist_url: &str) -> RegistryVersion {
    RegistryVersion {
        version: manifest.version.unwrap_or_else(|| "0.0.0".to_string()),
        dependencies: manifest.dependencies,
        optional_dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        peer_dependencies_meta: BTreeMap::new(),
        bundled_dependencies: None,
        bundle_dependencies: None,
        dist: RegistryDist {
            tarball: dist_url.to_string(),
            integrity: None,
        },
        os: vec![],
        cpu: vec![],
        bin: None,
    }
}

pub fn encode_package_name(name: &str) -> String {
    if name.starts_with('@') {
        name.replace('/', "%2F")
    } else {
        name.to_string()
    }
}
