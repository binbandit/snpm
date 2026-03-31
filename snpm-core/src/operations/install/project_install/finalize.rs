use super::patches::apply_patches;
use super::plan::ProjectInstallPlan;
use crate::console;
use crate::lifecycle;
use crate::linker;
use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Project, Result, SnpmConfig, Workspace};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::operations::install::utils::{
    IntegrityState, build_project_integrity_state, can_any_scripts_run, write_integrity_file,
};
use crate::operations::install::workspace::link_local_workspace_deps;

pub(super) fn finalize_install(
    config: &SnpmConfig,
    project: &Project,
    plan: &ProjectInstallPlan,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    include_dev: bool,
    precomputed_integrity: Option<IntegrityState>,
) -> Result<()> {
    let link_start = Instant::now();

    linker::link(
        config,
        plan.workspace.as_ref(),
        project,
        graph,
        store_paths,
        include_dev,
    )?;

    link_local_workspace_deps(
        project,
        plan.workspace.as_ref(),
        &plan.local_deps,
        &plan.local_dev_deps,
        &plan.local_optional_deps,
        include_dev,
    )?;

    let patches_applied = apply_patches(project, store_paths)?;
    if patches_applied > 0 {
        console::verbose(&format!("applied {} patches", patches_applied));
    }

    let integrity_state = match precomputed_integrity {
        Some(state) => state,
        None => build_project_integrity_state(project, graph)?,
    };
    write_integrity_file(project, &integrity_state)?;

    console::verbose(&format!(
        "linking completed in {:.3}s",
        link_start.elapsed().as_secs_f64()
    ));

    Ok(())
}

pub(super) fn run_install_scripts(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    project_root: &Path,
    early_exit: bool,
) -> Result<Vec<String>> {
    if early_exit {
        console::verbose("skipping install scripts (early exit - node_modules is fresh)");
        return Ok(Vec::new());
    }

    if !can_any_scripts_run(config, workspace) {
        console::verbose("skipping install scripts (no scripts can run based on config)");
        return Ok(Vec::new());
    }

    let blocked = lifecycle::run_install_scripts(config, workspace, project_root)?;
    lifecycle::run_project_scripts(config, workspace, project_root)?;
    Ok(blocked)
}
