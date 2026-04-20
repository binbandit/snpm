use crate::console;
use crate::lockfile;
use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Result, SnpmConfig};

use reqwest::Client;
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::super::utils::{
    InstallScenario, check_store_cache, materialize_missing_packages, materialize_store,
};
use super::plan::WorkspaceInstallPlan;
use super::resolution::resolve_workspace_deps;

pub(super) struct WorkspaceGraphLoad {
    pub(super) graph: ResolutionGraph,
    pub(super) store_paths_map: BTreeMap<PackageId, PathBuf>,
    pub(super) wrote_lockfile: bool,
}

pub(super) async fn load_workspace_graph(
    config: &SnpmConfig,
    registry_client: &Client,
    plan: &WorkspaceInstallPlan,
    include_dev: bool,
    force: bool,
) -> Result<WorkspaceGraphLoad> {
    let mut workspace_graph = match plan.scenario {
        InstallScenario::Hot => load_hot_graph(plan),
        InstallScenario::WarmLinkOnly => load_warm_link_graph(config, plan),
        InstallScenario::WarmPartialCache => {
            load_warm_partial_graph(config, registry_client, plan).await
        }
        InstallScenario::Cold => {
            load_cold_graph(config, registry_client, plan, include_dev, force).await
        }
    }?;

    if include_dev
        && !workspace_graph.wrote_lockfile
        && plan.setup.has_compatible_lockfile()
        && !plan.setup.lockfile_path.is_file()
    {
        lockfile::write(
            &plan.setup.lockfile_path,
            &workspace_graph.graph,
            &plan.setup.root_specs.optional,
        )?;
        console::step("Saved lockfile");
        workspace_graph.wrote_lockfile = true;
    }

    Ok(workspace_graph)
}

fn load_hot_graph(plan: &WorkspaceInstallPlan) -> Result<WorkspaceGraphLoad> {
    console::step("Using cached install");

    Ok(WorkspaceGraphLoad {
        graph: lockfile::to_graph(existing_lockfile(plan, "Hot")?),
        store_paths_map: BTreeMap::new(),
        wrote_lockfile: false,
    })
}

fn load_warm_link_graph(
    config: &SnpmConfig,
    plan: &WorkspaceInstallPlan,
) -> Result<WorkspaceGraphLoad> {
    let graph = lockfile::to_graph(existing_lockfile(plan, "WarmLinkOnly")?);
    let cache_check = check_store_cache(config, &graph);

    console::step_with_count("Using cached packages", cache_check.cached.len());

    Ok(WorkspaceGraphLoad {
        graph,
        store_paths_map: cache_check.cached,
        wrote_lockfile: false,
    })
}

async fn load_warm_partial_graph(
    config: &SnpmConfig,
    registry_client: &Client,
    plan: &WorkspaceInstallPlan,
) -> Result<WorkspaceGraphLoad> {
    let graph = lockfile::to_graph(existing_lockfile(plan, "WarmPartialCache")?);
    let cache_check = check_store_cache(config, &graph);
    let mut store_paths_map = cache_check.cached;

    if !cache_check.missing.is_empty() {
        console::step("Downloading missing packages");
        let downloaded =
            materialize_missing_packages(config, &cache_check.missing, registry_client).await?;
        store_paths_map.extend(downloaded);
    }

    console::step_with_count("Resolved and extracted", store_paths_map.len());

    Ok(WorkspaceGraphLoad {
        graph,
        store_paths_map,
        wrote_lockfile: false,
    })
}

async fn load_cold_graph(
    config: &SnpmConfig,
    registry_client: &Client,
    plan: &WorkspaceInstallPlan,
    include_dev: bool,
    force: bool,
) -> Result<WorkspaceGraphLoad> {
    console::step("Resolving workspace dependencies");

    let mut store_paths_map = BTreeMap::new();
    let graph = resolve_workspace_deps(
        config,
        registry_client,
        &plan.setup.root_dependencies,
        &plan.setup.root_protocols,
        &plan.setup.optional_root_names,
        force,
        &mut store_paths_map,
    )
    .await?;

    if store_paths_map.is_empty() && !graph.packages.is_empty() {
        store_paths_map = materialize_store(config, &graph, registry_client).await?;
    }

    if include_dev {
        lockfile::write(
            &plan.setup.lockfile_path,
            &graph,
            &plan.setup.root_specs.optional,
        )?;
        console::step("Saved lockfile");
    }

    console::step_with_count("Resolved, downloaded and extracted", store_paths_map.len());

    Ok(WorkspaceGraphLoad {
        graph,
        store_paths_map,
        wrote_lockfile: include_dev,
    })
}

fn existing_lockfile<'a>(
    plan: &'a WorkspaceInstallPlan,
    scenario: &str,
) -> crate::Result<&'a lockfile::Lockfile> {
    plan.existing_lockfile
        .as_ref()
        .ok_or_else(|| crate::SnpmError::Lockfile {
            path: plan.setup.lockfile_path.clone(),
            reason: format!("{scenario} scenario requires existing lockfile"),
        })
}
