use super::manifest::link_bins_from_bundled_pkg;
use crate::Result;
use crate::registry::BundledDependencies;
use crate::resolve::{PackageId, ResolutionGraph, ResolvedPackage};
use crate::store::read_package_metadata_lossy;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub fn link_bundled_bins_recursive(
    graph: &ResolutionGraph,
    linked: &BTreeMap<PackageId, PathBuf>,
) -> Result<()> {
    for (id, destination) in linked {
        let bundled = bundled_dependency_names(graph.packages.get(id), destination);
        if !bundled.is_empty() {
            link_bundled_bins(destination, &bundled)?;
        }
    }

    Ok(())
}

fn link_bundled_bins(pkg_dest: &Path, bundled: &BTreeSet<String>) -> Result<()> {
    let bundled_modules = pkg_dest.join("node_modules");
    if !bundled_modules.is_dir() {
        return Ok(());
    }

    let bin_dir = bundled_modules.join(".bin");
    for name in bundled {
        let package_path = bundled_modules.join(name);
        let Ok(metadata) = package_path.symlink_metadata() else {
            continue;
        };

        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            continue;
        }

        link_bins_from_bundled_pkg(&package_path, &bin_dir, name)?;
    }

    Ok(())
}

fn bundled_dependency_names(
    package: Option<&ResolvedPackage>,
    package_root: &Path,
) -> BTreeSet<String> {
    let bundled = package
        .and_then(|package| package.bundled_dependencies.clone())
        .or_else(|| {
            read_package_metadata_lossy(package_root)
                .and_then(|metadata| metadata.bundled_dependencies)
        });

    match bundled {
        Some(BundledDependencies::List(list)) => list.into_iter().collect(),
        Some(BundledDependencies::All(true)) => package
            .map(|package| package.dependencies.keys().cloned().collect())
            .unwrap_or_default(),
        _ => BTreeSet::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::link_bundled_bins_recursive;
    use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage};
    use crate::store::PACKAGE_METADATA_FILE;

    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn link_bundled_bins_recursive_uses_store_metadata_when_graph_lacks_bundle_info() {
        let dir = tempdir().unwrap();
        let package_id = PackageId {
            name: "parent".to_string(),
            version: "1.0.0".to_string(),
        };
        let dependency_id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };
        let package_root = dir.path().join("node_modules/parent");
        let bundled_root = package_root.join("node_modules/dep");
        fs::create_dir_all(&bundled_root).unwrap();
        fs::write(bundled_root.join("cli.js"), "#!/usr/bin/env node\n").unwrap();
        fs::write(
            package_root.join(PACKAGE_METADATA_FILE),
            r#"{ "bundledDependencies": ["dep"] }"#,
        )
        .unwrap();
        fs::write(
            bundled_root.join(PACKAGE_METADATA_FILE),
            r#"{ "hasBin": true, "bin": "cli.js" }"#,
        )
        .unwrap();

        let graph = ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::new(),
            },
            packages: BTreeMap::from([(
                package_id.clone(),
                ResolvedPackage {
                    id: package_id.clone(),
                    tarball: String::new(),
                    integrity: None,
                    dependencies: BTreeMap::from([("dep".to_string(), dependency_id)]),
                    peer_dependencies: BTreeMap::new(),
                    bundled_dependencies: None,
                    has_bin: false,
                    bin: None,
                },
            )]),
        };
        let linked = BTreeMap::from([(package_id, package_root.clone())]);

        link_bundled_bins_recursive(&graph, &linked).unwrap();

        assert!(package_root.join("node_modules/.bin/dep").exists());
    }
}
