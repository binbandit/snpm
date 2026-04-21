use super::super::types::{PackageId, ResolvedPackage};
use crate::registry::RegistryVersion;
use crate::{Result, SnpmError};

use std::collections::{BTreeMap, BTreeSet};

pub(super) fn ensure_platform_compatible(
    name: &str,
    range: &str,
    version_meta: &RegistryVersion,
) -> Result<()> {
    if crate::platform::is_compatible(&version_meta.os, &version_meta.cpu) {
        return Ok(());
    }

    Err(SnpmError::ResolutionFailed {
        name: name.to_string(),
        range: range.to_string(),
        reason: "package is not compatible with current OS/CPU".to_string(),
    })
}

pub(super) fn build_placeholder(id: &PackageId, version_meta: &RegistryVersion) -> ResolvedPackage {
    ResolvedPackage {
        id: id.clone(),
        tarball: version_meta.dist.tarball.clone(),
        integrity: version_meta.dist.integrity.clone(),
        dependencies: BTreeMap::new(),
        peer_dependencies: required_peer_dependencies(version_meta),
        bundled_dependencies: version_meta.get_bundled_dependencies().cloned(),
        has_bin: version_meta.has_bin(),
        bin: version_meta.bin_definition(),
    }
}

fn required_peer_dependencies(version_meta: &RegistryVersion) -> BTreeMap<String, String> {
    let mut peer_dependencies = BTreeMap::new();

    for (name, range) in &version_meta.peer_dependencies {
        let is_optional = version_meta
            .peer_dependencies_meta
            .get(name)
            .map(|meta| meta.optional)
            .unwrap_or(false);

        if !is_optional {
            peer_dependencies.insert(name.clone(), range.clone());
        }
    }

    peer_dependencies
}

pub(super) fn bundled_dependency_names(version_meta: &RegistryVersion) -> BTreeSet<String> {
    version_meta
        .get_bundled_dependencies()
        .map(|bundled| bundled.to_set(&version_meta.dependencies))
        .unwrap_or_default()
}
