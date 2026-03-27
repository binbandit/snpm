pub mod bins;
pub mod fs;
pub mod hoist;

use crate::HoistingMode;
use crate::resolve::{PackageId, ResolutionGraph, RootDependency};
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bins::link_bins;
use fs::{
    copy_dir, ensure_parent_dir, link_dir, package_node_modules, symlink_dir_entry,
    symlink_is_correct,
};
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

    let virtual_store_paths =
        populate_virtual_store(&root_node_modules, graph, store_paths, config)?;

    link_virtual_dependencies(&virtual_store_paths, graph)?;

    let root_deps_to_link = filter_root_dependencies(project, graph, include_dev);

    link_root_dependencies(&root_deps_to_link, &virtual_store_paths, &root_node_modules)?;

    link_root_bins(&root_deps_to_link, &root_node_modules, graph)?;

    if let HoistingMode::None = effective_hoisting(config, workspace) {
        return Ok(());
    }

    hoist_packages(
        config,
        workspace,
        project,
        graph,
        store_paths,
        effective_hoisting(config, workspace),
    )
}

fn populate_virtual_store(
    root_node_modules: &Path,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    config: &SnpmConfig,
) -> Result<Arc<BTreeMap<PackageId, PathBuf>>> {
    let virtual_store_dir = root_node_modules.join(".snpm");
    let packages: Vec<_> = graph.packages.iter().collect();

    let results: Vec<Result<(PackageId, PathBuf)>> = packages
        .par_iter()
        .map(|(id, _package)| -> Result<(PackageId, PathBuf)> {
            let safe_name = id.name.replace('/', "+");
            let virtual_id_dir = virtual_store_dir.join(format!("{}@{}", safe_name, id.version));
            let package_location = virtual_id_dir.join("node_modules").join(&id.name);

            let store_path = store_paths.get(id).ok_or_else(|| SnpmError::StoreMissing {
                name: id.name.clone(),
                version: id.version.clone(),
            })?;

            if let Ok(meta) = package_location.symlink_metadata() {
                if meta.is_dir() && !meta.file_type().is_symlink() {
                    return Ok(((*id).clone(), package_location));
                }
                std::fs::remove_file(&package_location).ok();
                std::fs::remove_dir_all(&package_location).ok();
            }

            ensure_parent_dir(&package_location)?;

            link_dir(config, store_path, &package_location)?;

            Ok(((*id).clone(), package_location))
        })
        .collect();

    let mut map = BTreeMap::new();
    for result in results {
        let (id, path) = result?;
        map.insert(id, path);
    }
    Ok(Arc::new(map))
}

fn link_virtual_dependencies(
    virtual_store_paths: &Arc<BTreeMap<PackageId, PathBuf>>,
    graph: &ResolutionGraph,
) -> Result<()> {
    let packages: Vec<_> = graph.packages.iter().collect();

    packages
        .par_iter()
        .try_for_each(|(id, package)| -> Result<()> {
            let package_location =
                virtual_store_paths
                    .get(id)
                    .ok_or_else(|| SnpmError::GraphMissing {
                        name: id.name.clone(),
                        version: id.version.clone(),
                    })?;
            let package_node_modules = package_node_modules(package_location, &id.name)
                .ok_or_else(|| SnpmError::GraphMissing {
                    name: id.name.clone(),
                    version: id.version.clone(),
                })?;

            for (dep_name, dep_id) in &package.dependencies {
                let dep_target =
                    virtual_store_paths
                        .get(dep_id)
                        .ok_or_else(|| SnpmError::GraphMissing {
                            name: dep_id.name.clone(),
                            version: dep_id.version.clone(),
                        })?;
                let dep_link = package_node_modules.join(dep_name);

                if symlink_is_correct(&dep_link, dep_target) {
                    continue;
                }

                std::fs::remove_file(&dep_link).ok();
                std::fs::remove_dir_all(&dep_link).ok();

                ensure_parent_dir(&dep_link)?;

                symlink_dir_entry(dep_target, &dep_link)
                    .or_else(|_| copy_dir(dep_target, &dep_link))?;
            }
            Ok(())
        })
}

fn filter_root_dependencies<'a>(
    project: &Project,
    graph: &'a ResolutionGraph,
    include_dev: bool,
) -> Vec<(&'a String, &'a RootDependency)> {
    let deps = &project.manifest.dependencies;
    let dev_deps = &project.manifest.dev_dependencies;
    let optional_deps = &project.manifest.optional_dependencies;

    graph
        .root
        .dependencies
        .iter()
        .filter(|(name, _dep)| {
            if !deps.contains_key(*name)
                && !dev_deps.contains_key(*name)
                && !optional_deps.contains_key(*name)
            {
                return false;
            }
            let only_dev = dev_deps.contains_key(*name) && !deps.contains_key(*name);
            if !include_dev && only_dev {
                return false;
            }
            true
        })
        .collect()
}

fn link_root_dependencies(
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

fn link_root_bins(
    root_deps: &[(&String, &RootDependency)],
    root_node_modules: &Path,
    graph: &ResolutionGraph,
) -> Result<()> {
    root_deps.par_iter().for_each(|(name, dep)| {
        // Skip packages that don't declare any bin entries
        if let Some(pkg) = graph.packages.get(&dep.resolved)
            && !pkg.has_bin
        {
            return;
        }
        let dest = root_node_modules.join(name);
        if let Err(e) = link_bins(&dest, root_node_modules, name) {
            crate::console::warn(&format!("failed to link bins for {}: {}", name, e));
        }
    });
    Ok(())
}
