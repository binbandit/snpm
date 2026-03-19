use super::fs::copy_dir;
use super::fs::link_dir_fast;
use crate::resolve::{PackageId, ResolutionGraph};
use crate::{HoistingMode, Project, Result, SnpmConfig, SnpmError, Workspace, lifecycle};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn hoist_packages(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    project: &Project,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
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

        if dest.exists() {
            return Ok(());
        }

        link_shallow_package(config, workspace, id, &dest, store_paths)
    })?;

    Ok(())
}

fn link_shallow_package(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    id: &PackageId,
    dest: &Path,
    store_paths: &BTreeMap<PackageId, PathBuf>,
) -> Result<()> {
    if dest.exists() {
        return Ok(());
    }

    let store_root = store_paths.get(id).ok_or_else(|| SnpmError::StoreMissing {
        name: id.name.clone(),
        version: id.version.clone(),
    })?;

    let scripts_allowed = lifecycle::is_dep_script_allowed(config, workspace, &id.name);

    if scripts_allowed {
        copy_dir(store_root, dest)?;
    } else {
        link_dir_fast(config, store_root, dest)?;
    }

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
