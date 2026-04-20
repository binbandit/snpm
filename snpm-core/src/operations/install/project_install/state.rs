use super::cold::resolve_cold_install;
use super::plan::ProjectInstallPlan;
use crate::SnpmError;
use crate::console;
use crate::lockfile;
use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Project, Result, SnpmConfig};

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::operations::install::utils::{
    InstallOptions, InstallScenario, IntegrityState, ScenarioResult, detect_install_scenario,
    materialize_missing_packages,
};

pub(super) struct ResolvedInstall {
    pub scenario: InstallScenario,
    pub graph: ResolutionGraph,
    pub store_paths: BTreeMap<PackageId, PathBuf>,
    pub integrity_state: Option<IntegrityState>,
}

pub(super) async fn resolve_install_state(
    config: &SnpmConfig,
    project: &Project,
    plan: &ProjectInstallPlan,
    options: &InstallOptions,
    registry_client: &reqwest::Client,
) -> Result<ResolvedInstall> {
    let scenario_result = if options.include_dev && plan.additions.is_empty() {
        detect_install_scenario(
            project,
            &plan.lockfile_path,
            &plan.manifest_root,
            &plan.root_specs.optional,
            config,
            options.force,
        )
    } else {
        ScenarioResult::cold()
    };

    let ScenarioResult {
        cache_check,
        graph,
        integrity_state,
        scenario,
        ..
    } = scenario_result;
    let mut store_paths = BTreeMap::new();

    let graph = match scenario {
        InstallScenario::Hot => require_graph(graph, "Hot")?,
        InstallScenario::WarmLinkOnly => {
            let graph = require_graph(graph, "WarmLinkOnly")?;
            let cache_check =
                require_cache(cache_check, "WarmLinkOnly scenario requires cache state")?;

            store_paths = cache_check.cached;
            console::verbose(&format!(
                "warm link-only path: {} packages from cache",
                store_paths.len()
            ));
            console::step_with_count("Using cached packages", store_paths.len());

            graph
        }
        InstallScenario::WarmPartialCache => {
            let graph = require_graph(graph, "WarmPartialCache")?;
            let cache_check = require_cache(
                cache_check,
                "WarmPartialCache scenario requires cache state",
            )?;
            let cached_count = cache_check.cached.len();
            let missing_count = cache_check.missing.len();

            console::verbose(&format!(
                "warm partial-cache path: {} cached, {} to download",
                cached_count, missing_count
            ));

            store_paths = cache_check.cached;

            if !cache_check.missing.is_empty() {
                console::step("Downloading missing packages");
                let materialize_start = Instant::now();
                let downloaded =
                    materialize_missing_packages(config, &cache_check.missing, registry_client)
                        .await?;

                console::verbose(&format!(
                    "downloaded {} missing packages in {:.3}s",
                    downloaded.len(),
                    materialize_start.elapsed().as_secs_f64()
                ));

                store_paths.extend(downloaded);
            }

            console::step_with_count("Resolved, downloaded and extracted", store_paths.len());
            graph
        }
        InstallScenario::Cold => {
            let (graph, resolved_store_paths) =
                resolve_cold_install(config, registry_client, plan, options.force).await?;

            store_paths = resolved_store_paths;

            if options.include_dev {
                lockfile::write(&plan.lockfile_path, &graph, &plan.root_specs.optional)?;
            }

            console::step_with_count("Resolved, downloaded and extracted", store_paths.len());
            graph
        }
    };

    if options.include_dev {
        console::step("Saved lockfile");
    }

    Ok(ResolvedInstall {
        scenario,
        graph,
        store_paths,
        integrity_state,
    })
}

fn require_graph(graph: Option<ResolutionGraph>, scenario: &str) -> Result<ResolutionGraph> {
    graph.ok_or_else(|| SnpmError::Internal {
        reason: format!("{scenario} scenario requires a precomputed graph"),
    })
}

fn require_cache<T>(cache_check: Option<T>, message: &str) -> Result<T> {
    cache_check.ok_or_else(|| SnpmError::Internal {
        reason: message.to_string(),
    })
}
