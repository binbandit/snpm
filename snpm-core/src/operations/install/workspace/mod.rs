mod finalize;
mod graph;
mod linking;
mod patches;
mod plan;
mod resolution;
mod root_specs;
mod setup;

pub use linking::link_local_workspace_deps;
pub use root_specs::{
    collect_workspace_root_deps, collect_workspace_root_specs, insert_workspace_root_dep,
    validate_workspace_spec,
};

use crate::console;
use crate::{Result, SnpmConfig, Workspace, http};

use std::time::Instant;

use super::utils::InstallResult;
use finalize::finalize_workspace_install;
use graph::load_workspace_graph;
use plan::plan_workspace_install;

pub async fn install_workspace(
    config: &SnpmConfig,
    workspace: &mut Workspace,
    include_dev: bool,
    frozen_lockfile: bool,
    force: bool,
) -> Result<InstallResult> {
    let started = Instant::now();

    if workspace.projects.is_empty() {
        return Ok(InstallResult {
            package_count: 0,
            elapsed_seconds: 0.0,
        });
    }

    let registry_client = http::create_client()?;
    let plan = plan_workspace_install(config, workspace, include_dev, frozen_lockfile, force)?;

    if plan.setup.root_dependencies.is_empty() {
        console::summary(0, 0.0);
        return Ok(InstallResult {
            package_count: 0,
            elapsed_seconds: 0.0,
        });
    }

    let workspace_graph =
        load_workspace_graph(config, &registry_client, &plan, include_dev, force).await?;

    let blocked_scripts = finalize_workspace_install(
        config,
        workspace,
        &workspace_graph.graph,
        &workspace_graph.store_paths_map,
        include_dev,
        plan.scenario,
    )?;

    if include_dev {
        console::step("Saved lockfile");
    }

    console::clear_steps(if include_dev { 4 } else { 3 });

    let seconds = started.elapsed().as_secs_f32();
    let package_count = workspace_graph.graph.packages.len();

    console::summary(package_count, seconds);

    if !blocked_scripts.is_empty() {
        println!();
        console::blocked_scripts(&blocked_scripts);
    }

    Ok(InstallResult {
        package_count,
        elapsed_seconds: seconds,
    })
}
