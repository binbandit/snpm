mod cold;
mod finalize;
mod patches;
mod plan;
mod report;
mod state;

use crate::console;
use crate::{Project, Result, SnpmConfig, http};

use std::time::Instant;

use super::manifest::write_manifest;
use super::utils::{InstallOptions, InstallResult, InstallScenario};
use finalize::{finalize_install, run_install_scripts};
use plan::{prepare_install_plan, validate_frozen_lockfile};
use report::print_install_changes;
use state::resolve_install_state;

pub async fn install(
    config: &SnpmConfig,
    project: &mut Project,
    options: InstallOptions,
) -> Result<InstallResult> {
    let started = Instant::now();

    console::verbose(&format!(
        "install start: root={} requested=[{}] dev={} include_dev={} frozen_lockfile={} force={}",
        project.root.display(),
        options.requested.join(", "),
        options.dev,
        options.include_dev,
        options.frozen_lockfile.as_str(),
        options.force,
    ));

    let plan = prepare_install_plan(project, &options)?;

    console::verbose(&format!(
        "workspace_root={} overrides={} catalog_local={}",
        plan.workspace_root_label(),
        plan.overrides.len(),
        plan.catalog
            .as_ref()
            .map(|catalog| catalog.catalog.len())
            .unwrap_or(0),
    ));
    console::verbose(&format!(
        "manifest_root_deps={} root_deps={} additions={}",
        plan.manifest_root.len(),
        plan.root_dependencies.len(),
        plan.additions.len()
    ));
    console::verbose(&format!(
        "lockfile_path={} source={} exists={} fresh_install={}",
        plan.lockfile_path.display(),
        plan.lockfile_source_label(),
        plan.lockfile_path.is_file(),
        plan.is_fresh_install
    ));

    validate_frozen_lockfile(config, &options, &plan)?;

    let registry_client = http::create_client()?;
    let resolved =
        resolve_install_state(config, project, &plan, &options, &registry_client).await?;

    write_manifest(
        project,
        &resolved.graph,
        &plan.additions,
        options.dev,
        plan.workspace.as_ref(),
        plan.catalog.as_ref(),
    )?;

    let early_exit = matches!(resolved.scenario, InstallScenario::Hot);
    if early_exit {
        console::verbose("using early exit path (warm path optimization)");
    } else {
        finalize_install(
            config,
            project,
            &plan,
            &resolved.graph,
            &resolved.store_paths,
            options.include_dev,
            resolved.integrity_state,
        )?;
    }

    let scripts_start = Instant::now();
    let blocked_scripts =
        run_install_scripts(config, plan.workspace.as_ref(), &project.root, early_exit)?;

    console::verbose(&format!(
        "install scripts completed in {:.3}s (blocked_scripts={})",
        scripts_start.elapsed().as_secs_f64(),
        blocked_scripts.len()
    ));

    console::clear_steps(step_count_for_install(
        resolved.scenario,
        resolved.wrote_lockfile,
    ));
    print_install_changes(&resolved.graph, &plan, &options);

    let elapsed_seconds = started.elapsed().as_secs_f32();
    let package_count = resolved.graph.packages.len();

    if !options.silent_summary {
        console::summary(package_count, elapsed_seconds);
    }

    console::verbose(&format!(
        "install completed in {:.3}s (packages={} store_paths={} additions={} is_fresh_install={} blocked_scripts={})",
        elapsed_seconds,
        package_count,
        resolved.store_paths.len(),
        plan.additions.len(),
        plan.is_fresh_install,
        blocked_scripts.len()
    ));

    if !blocked_scripts.is_empty() {
        println!();
        console::blocked_scripts(&blocked_scripts);
    }

    Ok(InstallResult {
        package_count,
        elapsed_seconds,
    })
}

fn step_count_for_install(scenario: InstallScenario, wrote_lockfile: bool) -> usize {
    let scenario_steps = match scenario {
        InstallScenario::Hot => 0,
        InstallScenario::WarmLinkOnly => 1,
        InstallScenario::WarmPartialCache | InstallScenario::Cold => 2,
    };

    scenario_steps + usize::from(wrote_lockfile)
}
