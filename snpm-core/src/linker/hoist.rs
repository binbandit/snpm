use super::fs::{copy_dir, ensure_parent_dir, symlink_dir_entry, symlink_is_correct};
use crate::resolve::{PackageId, ResolutionGraph};
use crate::{HoistingMode, Project, Result, SnpmConfig, SnpmError, Workspace};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn hoist_packages(
    project: &Project,
    graph: &ResolutionGraph,
    virtual_store_paths: &Arc<BTreeMap<PackageId, PathBuf>>,
    mode: HoistingMode,
) -> Result<()> {
    if matches!(mode, HoistingMode::None) {
        return Ok(());
    }

    let root_node_modules = project.root.join("node_modules");

    let mut ids_by_name: BTreeMap<&str, Vec<&PackageId>> = BTreeMap::new();

    for id in graph.packages.keys() {
        ids_by_name.entry(&id.name).or_default().push(id);
    }

    let to_hoist: Vec<_> = ids_by_name
        .iter()
        .filter_map(|(name, ids)| {
            let should_hoist = match mode {
                HoistingMode::None => false,
                HoistingMode::SingleVersion => ids.len() == 1,
                HoistingMode::All => !ids.is_empty(),
            };

            if should_hoist {
                Some((*name, ids[0]))
            } else {
                None
            }
        })
        .collect();

    to_hoist.par_iter().try_for_each(|(name, id)| {
        let dest = root_node_modules.join(name);
        link_hoisted_package(name, id, &dest, virtual_store_paths)
    })?;

    Ok(())
}

fn link_hoisted_package(
    name: &str,
    id: &PackageId,
    dest: &Path,
    virtual_store_paths: &Arc<BTreeMap<PackageId, PathBuf>>,
) -> Result<()> {
    let target = virtual_store_paths
        .get(id)
        .ok_or_else(|| SnpmError::StoreMissing {
            name: id.name.clone(),
            version: id.version.clone(),
        })?;

    if symlink_is_correct(dest, target) {
        return Ok(());
    }

    std::fs::remove_file(dest).ok();
    std::fs::remove_dir_all(dest).ok();

    if name.contains('/') {
        ensure_parent_dir(dest)?;
    }

    symlink_dir_entry(target, dest).or_else(|_| copy_dir(target, dest))?;

    Ok(())
}

pub fn effective_hoisting(config: &SnpmConfig, workspace: Option<&Workspace>) -> HoistingMode {
    if let Some(ws) = workspace
        && let Some(value) = ws.config.hoisting.as_deref()
        && let Some(mode) = HoistingMode::parse(value)
    {
        return mode;
    }

    config.hoisting
}

#[cfg(test)]
mod tests {
    use super::hoist_packages;
    use crate::project::Manifest;
    use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage};
    use crate::{HoistingMode, Project};

    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn make_project(root: PathBuf) -> Project {
        Project {
            manifest_path: root.join("package.json"),
            root,
            manifest: Manifest {
                name: Some("app".to_string()),
                version: Some("1.0.0".to_string()),
                private: false,
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
                optional_dependencies: BTreeMap::new(),
                scripts: BTreeMap::new(),
                resolutions: BTreeMap::new(),
                files: None,
                bin: None,
                main: None,
                pnpm: None,
                snpm: None,
                workspaces: None,
            },
        }
    }

    #[cfg(unix)]
    #[test]
    fn hoist_packages_symlinks_to_virtual_store_entries() {
        let dir = tempdir().unwrap();
        let project = make_project(dir.path().join("project"));
        fs::create_dir_all(project.root.join("node_modules")).unwrap();

        let id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };
        let virtual_target = dir.path().join(".snpm/dep@1.0.0/node_modules/dep");
        fs::create_dir_all(&virtual_target).unwrap();
        fs::write(
            virtual_target.join("package.json"),
            r#"{"name":"dep","version":"1.0.0"}"#,
        )
        .unwrap();

        let pkg = ResolvedPackage {
            id: id.clone(),
            tarball: String::new(),
            integrity: None,
            dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: None,
            has_bin: false,
            bin: None,
        };
        let graph = ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::new(),
            },
            packages: BTreeMap::from([(id.clone(), pkg)]),
        };
        let virtual_store_paths = Arc::new(BTreeMap::from([(id.clone(), virtual_target.clone())]));

        hoist_packages(&project, &graph, &virtual_store_paths, HoistingMode::All).unwrap();

        let hoisted = project.root.join("node_modules/dep");
        assert!(hoisted.exists());
        assert!(hoisted.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(fs::read_link(&hoisted).unwrap(), virtual_target);
    }
}
