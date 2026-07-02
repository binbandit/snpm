use crate::resolve::{PackageId, ResolutionGraph, RootDependency};
use crate::{Result, SnpmError};

use rayon::prelude::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::bins::{link_bins, link_known_bins};
use super::fs::{
    copy_dir, ensure_parent_dir, remove_symlink, symlink_dir_entry, symlink_is_correct,
};

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
        // `has_bin=false` is not authoritative: yarn-classic and yarn-berry
        // lockfile imports set it to `false` unconditionally because the
        // lockfile format doesn't carry the field. Falling through to
        // `link_bins` reads the actual package.json (cheap, already in fs
        // cache from extraction) — a no-op when there really is no bin.
        let result = match graph.packages.get(&dep.resolved) {
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

/// Remove root `node_modules` entries that snpm created for packages no
/// longer part of the install set. Without this, `snpm remove` (and a
/// prod-only install after adding dev deps) leaves stale links behind
/// forever — node_modules only ever grows.
///
/// Only symlinks pointing into one of snpm's virtual-store roots are
/// candidates: entries created by `snpm link`, `file:`/`link:` deps, and
/// workspace cross-links point elsewhere and are never touched. Real
/// directories from the copy_dir fallback (filesystems without symlink
/// support) carry no provenance and are also left alone — pruning there
/// would risk deleting user-created directories.
pub(super) fn prune_stale_root_entries(
    root_node_modules: &Path,
    keep: &std::collections::BTreeSet<String>,
    virtual_store_roots: &[PathBuf],
) {
    let Ok(entries) = std::fs::read_dir(root_node_modules) else {
        return;
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();

        if file_name.starts_with('@') && path.is_dir() && !path.is_symlink() {
            if let Ok(scoped) = std::fs::read_dir(&path) {
                for scoped_entry in scoped.flatten() {
                    let scoped_name = format!(
                        "{}/{}",
                        file_name,
                        scoped_entry.file_name().to_string_lossy()
                    );
                    prune_entry_if_stale(
                        &scoped_entry.path(),
                        &scoped_name,
                        keep,
                        virtual_store_roots,
                    );
                }
            }
            // Drop the scope dir once it's empty.
            let _ = std::fs::remove_dir(&path);
            continue;
        }

        prune_entry_if_stale(&path, &file_name, keep, virtual_store_roots);
    }

    prune_dangling_bin_launchers(&root_node_modules.join(".bin"));
}

fn prune_entry_if_stale(
    path: &Path,
    name: &str,
    keep: &std::collections::BTreeSet<String>,
    virtual_store_roots: &[PathBuf],
) {
    if keep.contains(name) || !path.is_symlink() {
        return;
    }

    let Ok(target) = std::fs::read_link(path) else {
        return;
    };

    if virtual_store_roots
        .iter()
        .any(|root| target.starts_with(root))
    {
        remove_symlink(path);
    }
}

/// Launchers whose target vanished (because the package's root link was
/// just pruned) are dead weight; drop them.
fn prune_dangling_bin_launchers(bin_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(bin_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_symlink() && std::fs::metadata(&path).is_err() {
            remove_symlink(&path);
        }
    }
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

    #[test]
    fn link_root_bins_falls_back_to_manifest_when_has_bin_is_false() {
        // Yarn lockfile imports leave `has_bin=false` and `bin=None` even
        // for packages with bin entries. link_root_bins must still discover
        // the bin from the extracted package.json — otherwise webpack/jest/
        // redux installs end up with empty .bin/ and root prepare scripts
        // fail with "husky: command not found".
        let dir = tempdir().unwrap();
        let root_node_modules = dir.path().join("node_modules");
        let package_dir = root_node_modules.join("husky");
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(package_dir.join("bin.js"), "#!/usr/bin/env node\n").unwrap();
        fs::write(
            package_dir.join("package.json"),
            r#"{"name":"husky","version":"9.0.0","bin":"bin.js"}"#,
        )
        .unwrap();

        let package_id = PackageId {
            name: "husky".to_string(),
            version: "9.0.0".to_string(),
        };
        let root_dep = RootDependency {
            requested: "^9.0.0".to_string(),
            resolved: package_id.clone(),
        };
        let graph = ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::from([("husky".to_string(), root_dep.clone())]),
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
                    // Simulate a yarn lockfile import: bin info is missing.
                    has_bin: false,
                    bin: None,
                },
            )]),
        };

        let dep_name = "husky".to_string();
        let deps = vec![(&dep_name, &root_dep)];
        link_root_bins(&deps, &root_node_modules, &graph).unwrap();

        assert!(
            root_node_modules.join(".bin/husky").exists(),
            "must fall through to manifest read when has_bin=false"
        );
    }
}
