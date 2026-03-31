use crate::console;
use crate::lifecycle;
use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Result, SnpmConfig, SnpmError, Workspace};

use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::super::utils::{
    InstallScenario, IntegrityState, build_workspace_integrity_state, can_any_scripts_run,
    compute_project_patch_hash, write_integrity_path,
};
use super::linking::{
    link_project_dependencies, link_store_dependencies, populate_virtual_store,
    rebuild_virtual_store_paths,
};
use super::patches::apply_workspace_patches;
use super::resolution::write_workspace_integrity;

pub(super) fn finalize_workspace_install(
    config: &SnpmConfig,
    workspace: &Workspace,
    graph: &ResolutionGraph,
    store_paths_map: &BTreeMap<PackageId, PathBuf>,
    include_dev: bool,
    scenario: InstallScenario,
) -> Result<Vec<String>> {
    let shared_virtual_store = workspace.root.join(".snpm");
    fs::create_dir_all(&shared_virtual_store).map_err(|source| SnpmError::WriteFile {
        path: shared_virtual_store.clone(),
        source,
    })?;

    let virtual_store_paths = if matches!(scenario, InstallScenario::Hot) {
        console::step("Validating workspace structure");
        rebuild_virtual_store_paths(&shared_virtual_store, graph)?
    } else {
        console::step("Linking workspace");
        populate_virtual_store(&shared_virtual_store, graph, store_paths_map, config)?
    };

    link_store_dependencies(&virtual_store_paths, graph)?;

    workspace.projects.par_iter().try_for_each(|project| {
        link_project_dependencies(project, workspace, graph, &virtual_store_paths, include_dev)
    })?;

    let patches_applied = apply_workspace_patches(workspace, store_paths_map)?;
    if patches_applied > 0 {
        console::verbose(&format!("applied {} workspace patches", patches_applied));
    }

    let blocked_scripts = run_workspace_scripts(config, workspace)?;
    let workspace_integrity = build_workspace_integrity_state(workspace, graph)?;
    write_workspace_integrity(&workspace.root, &workspace_integrity)?;
    write_project_integrity_files(workspace, &workspace_integrity)?;

    Ok(blocked_scripts)
}

fn run_workspace_scripts(config: &SnpmConfig, workspace: &Workspace) -> Result<Vec<String>> {
    if !can_any_scripts_run(config, Some(workspace)) {
        return Ok(Vec::new());
    }

    let roots: Vec<&Path> = workspace
        .projects
        .iter()
        .map(|project| project.root.as_path())
        .collect();
    let blocked = lifecycle::run_install_scripts_for_projects(config, Some(workspace), &roots)?;

    for project in &workspace.projects {
        lifecycle::run_project_scripts(config, Some(workspace), &project.root)?;
    }

    Ok(blocked)
}

fn write_project_integrity_files(
    workspace: &Workspace,
    workspace_integrity: &IntegrityState,
) -> Result<()> {
    for project in &workspace.projects {
        let project_integrity = IntegrityState {
            lockfile_hash: workspace_integrity.lockfile_hash.clone(),
            patch_hash: compute_project_patch_hash(project)?,
        };
        write_integrity_path(&project.root.join("node_modules"), &project_integrity)?;
    }

    Ok(())
}
