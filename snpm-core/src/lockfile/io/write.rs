use super::super::keys::package_key;
use super::super::types::{LOCKFILE_VERSION, LockPackage, LockRoot, LockRootDependency, Lockfile};
use super::binary::{encode_sidecar, sidecar_path, yaml_hash};
use crate::resolve::ResolutionGraph;
use crate::{Result, SnpmError};

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub fn write(
    path: &Path,
    graph: &ResolutionGraph,
    optional_root_specs: &BTreeMap<String, String>,
) -> Result<()> {
    let lockfile = Lockfile {
        version: LOCKFILE_VERSION,
        root: LockRoot {
            dependencies: build_root_dependencies(graph, optional_root_specs),
        },
        packages: build_packages(graph),
    };

    let data = serde_yaml::to_string(&lockfile).map_err(|source| SnpmError::LockfileWrite {
        path: path.to_path_buf(),
        source,
    })?;

    fs::write(path, &data).map_err(|source| SnpmError::WriteFile {
        path: path.to_path_buf(),
        source,
    })?;

    // Write the binary sidecar next to the YAML. We intentionally swallow any
    // error from this path: the sidecar is a fast-load cache and never the
    // source of truth. A read can always fall back to the YAML.
    if let Some(bytes) = encode_sidecar(&lockfile, yaml_hash(data.as_bytes())) {
        let _ = fs::write(sidecar_path(path), bytes);
    }

    Ok(())
}

fn build_root_dependencies(
    graph: &ResolutionGraph,
    optional_root_specs: &BTreeMap<String, String>,
) -> BTreeMap<String, LockRootDependency> {
    let mut root_dependencies = BTreeMap::new();

    for (name, dep) in &graph.root.dependencies {
        root_dependencies.insert(
            name.clone(),
            LockRootDependency {
                requested: dep.requested.clone(),
                package: (dep.resolved.name != *name).then(|| dep.resolved.name.clone()),
                version: Some(dep.resolved.version.clone()),
                optional: optional_root_specs.contains_key(name),
            },
        );
    }

    for (name, requested) in optional_root_specs {
        root_dependencies
            .entry(name.clone())
            .or_insert_with(|| LockRootDependency {
                requested: requested.clone(),
                package: None,
                version: None,
                optional: true,
            });
    }

    root_dependencies
}

fn build_packages(graph: &ResolutionGraph) -> BTreeMap<String, LockPackage> {
    let mut packages = BTreeMap::new();

    for package in graph.packages.values() {
        let dependencies = package
            .dependencies
            .iter()
            .map(|(name, dep_id)| (name.clone(), package_key(&dep_id.name, &dep_id.version)))
            .collect();

        packages.insert(
            package_key(&package.id.name, &package.id.version),
            LockPackage {
                name: package.id.name.clone(),
                version: package.id.version.clone(),
                tarball: package.tarball.clone(),
                integrity: package.integrity.clone(),
                dependencies,
                peer_dependencies: package.peer_dependencies.clone(),
                bundled_dependencies: package.bundled_dependencies.clone(),
                has_bin: package.has_bin,
                bin: package.bin.clone(),
            },
        );
    }

    packages
}
