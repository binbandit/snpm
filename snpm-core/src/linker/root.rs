use crate::resolve::{PackageId, ResolutionGraph, RootDependency};
use crate::{Result, SnpmError};

use rayon::prelude::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::bins::{link_bins, link_known_bins};
use super::fs::{copy_dir, ensure_parent_dir, symlink_dir_entry, symlink_is_correct};

pub(super) fn link_root_dependencies(
    root_deps: &[(&String, &RootDependency)],
    virtual_store_paths: &Arc<BTreeMap<PackageId, PathBuf>>,
    root_node_modules: &Path,
) -> Result<()> {
    root_deps
        .par_iter()
        .try_for_each(|(name, dep)| -> Result<()> {
            let id = &dep.resolved;
            let target = virtual_store_paths
                .get(id)
                .ok_or_else(|| SnpmError::GraphMissing {
                    name: id.name.clone(),
                    version: id.version.clone(),
                })?;

            let dest = root_node_modules.join(name);

            if symlink_is_correct(&dest, target) {
                return Ok(());
            }

            std::fs::remove_file(&dest).ok();
            std::fs::remove_dir_all(&dest).ok();

            if name.contains('/') {
                ensure_parent_dir(&dest)?;
            }

            symlink_dir_entry(target, &dest).or_else(|_| copy_dir(target, &dest))?;
            Ok(())
        })
}

pub(super) fn link_root_bins(
    root_deps: &[(&String, &RootDependency)],
    root_node_modules: &Path,
    graph: &ResolutionGraph,
) -> Result<()> {
    root_deps.par_iter().for_each(|(name, dep)| {
        let dest = root_node_modules.join(name);
        let result = match graph.packages.get(&dep.resolved) {
            Some(pkg) if !pkg.has_bin => return,
            Some(pkg) => pkg.bin.as_ref().map_or_else(
                || link_bins(&dest, root_node_modules, name),
                |bin| link_known_bins(&dest, root_node_modules, name, bin),
            ),
            None => link_bins(&dest, root_node_modules, name),
        };

        if let Err(error) = result {
            crate::console::warn(&format!("failed to link bins for {}: {}", name, error));
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::link_root_bins;
    use crate::project::BinField;
    use crate::resolve::{
        PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency,
    };

    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn link_root_bins_uses_graph_bin_metadata_without_manifest() {
        let dir = tempdir().unwrap();
        let root_node_modules = dir.path().join("node_modules");
        let package_dir = root_node_modules.join("tool");
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(package_dir.join("cli.js"), "#!/usr/bin/env node\n").unwrap();

        let package_id = PackageId {
            name: "tool".to_string(),
            version: "1.0.0".to_string(),
        };
        let root_dep = RootDependency {
            requested: "^1.0.0".to_string(),
            resolved: package_id.clone(),
        };
        let graph = ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::from([("tool".to_string(), root_dep.clone())]),
            },
            packages: BTreeMap::from([(
                package_id.clone(),
                ResolvedPackage {
                    id: package_id,
                    tarball: String::new(),
                    integrity: None,
                    dependencies: BTreeMap::new(),
                    peer_dependencies: BTreeMap::new(),
                    bundled_dependencies: None,
                    has_bin: true,
                    bin: Some(BinField::Single("cli.js".to_string())),
                },
            )]),
        };

        let dep_name = "tool".to_string();
        let deps = vec![(&dep_name, &root_dep)];
        link_root_bins(&deps, &root_node_modules, &graph).unwrap();

        assert!(root_node_modules.join(".bin/tool").exists());
    }
}
