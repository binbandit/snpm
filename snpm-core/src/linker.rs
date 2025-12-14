pub mod bins;
pub mod fs;
pub mod hoist;

use crate::resolve::{PackageId, ResolutionGraph};
use crate::{HoistingMode, lifecycle};
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use bins::{link_bins, link_bundled_bins_recursive};
use fs::{copy_dir, link_dir, symlink_dir_entry};
use hoist::{effective_hoisting, hoist_packages};

pub fn link(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    project: &Project,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    include_dev: bool,
) -> Result<()> {
    let root_node_modules = project.root.join("node_modules");

    std::fs::create_dir_all(&root_node_modules).map_err(|source| SnpmError::WriteFile {
        path: root_node_modules.clone(),
        source,
    })?;

    let deps = &project.manifest.dependencies;
    let dev_deps = &project.manifest.dev_dependencies;
    let hoisting = effective_hoisting(config, workspace);

    let mut linked: BTreeMap<PackageId, PathBuf> = BTreeMap::new();
    let mut root_bin_targets: BTreeMap<String, PathBuf> = BTreeMap::new();

    for (name, dep) in graph.root.dependencies.iter() {
        if !deps.contains_key(name) && !dev_deps.contains_key(name) {
            continue;
        }

        let only_dev = dev_deps.contains_key(name) && !deps.contains_key(name);
        if !include_dev && only_dev {
            continue;
        }

        let id = &dep.resolved;
        let dest = root_node_modules.join(name);
        let mut stack = BTreeSet::new();

        link_package(
            config,
            workspace,
            id,
            &dest,
            graph,
            store_paths,
            &mut stack,
            &mut linked,
        )?;

        root_bin_targets
            .entry(id.name.clone())
            .or_insert(dest.clone());
    }

    for (pkg_name, pkg_dest) in root_bin_targets.iter() {
        link_bins(pkg_dest, &root_node_modules, pkg_name)?;
    }

    link_bundled_bins_recursive(graph, &linked)?;

    if !matches!(hoisting, HoistingMode::None) {
        hoist_packages(config, workspace, project, graph, store_paths, hoisting)?;
    }

    Ok(())
}

fn link_package(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    id: &PackageId,
    dest: &Path,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    stack: &mut BTreeSet<PackageId>,
    linked: &mut BTreeMap<PackageId, PathBuf>,
) -> Result<()> {
    if let Some(existing) = linked.get(id) {
        if dest.exists() {
            std::fs::remove_dir_all(dest).map_err(|source| SnpmError::WriteFile {
                path: dest.to_path_buf(),
                source,
            })?;
        }

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        if let Err(_err) = symlink_dir_entry(existing, dest) {
            copy_dir(existing, dest)?;
        }

        return Ok(());
    }

    if stack.contains(id) {
        let store_root = store_paths.get(id).ok_or_else(|| SnpmError::StoreMissing {
            name: id.name.clone(),
            version: id.version.clone(),
        })?;

        if dest.exists() {
            std::fs::remove_dir_all(dest).map_err(|source| SnpmError::WriteFile {
                path: dest.to_path_buf(),
                source,
            })?;
        }

        let scripts_allowed = lifecycle::is_dep_script_allowed(config, workspace, &id.name);

        if scripts_allowed {
            copy_dir(store_root, dest)?;
        } else {
            link_dir(config, store_root, dest)?;
        }

        return Ok(());
    }

    stack.insert(id.clone());

    if dest.exists() {
        std::fs::remove_dir_all(dest).map_err(|source| SnpmError::WriteFile {
            path: dest.to_path_buf(),
            source,
        })?;
    }

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let store_root = store_paths.get(id).ok_or_else(|| SnpmError::StoreMissing {
        name: id.name.clone(),
        version: id.version.clone(),
    })?;

    let package = graph
        .packages
        .get(id)
        .ok_or_else(|| SnpmError::GraphMissing {
            name: id.name.clone(),
            version: id.version.clone(),
        })?;

    let scripts_allowed = lifecycle::is_dep_script_allowed(config, workspace, &id.name);
    let has_nested_deps = !package.dependencies.is_empty();

    if scripts_allowed || has_nested_deps {
        if scripts_allowed {
            copy_dir(store_root, dest)?;
        } else {
            link_dir(config, store_root, dest)?;
        }
    } else if symlink_dir_entry(store_root, dest).is_err() {
        link_dir(config, store_root, dest)?;
    }

    for (dep_name, dep_id) in package.dependencies.iter() {
        let node_modules = dest.join("node_modules");
        std::fs::create_dir_all(&node_modules).map_err(|source| SnpmError::WriteFile {
            path: node_modules.clone(),
            source,
        })?;
        let child_dest = node_modules.join(dep_name);

        link_package(
            config,
            workspace,
            dep_id,
            &child_dest,
            graph,
            store_paths,
            stack,
            linked,
        )?;
    }

    stack.remove(id);
    linked.insert(id.clone(), dest.to_path_buf());

    Ok(())
}
