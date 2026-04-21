use super::integrity::{build_project_integrity_state, check_integrity_file};
use super::layout_state::check_project_layout_state;
use super::store::check_store_cache;
use super::types::{InstallScenario, ScenarioResult};
use crate::console;
use crate::operations::install::utils::FrozenLockfileMode;
use crate::{Project, SnpmConfig, lockfile};
use std::collections::BTreeMap;
use std::path::Path;

pub fn detect_install_scenario(
    project: &Project,
    workspace: Option<&crate::Workspace>,
    lockfile_path: &Path,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
    config: &SnpmConfig,
    frozen_lockfile: FrozenLockfileMode,
    force: bool,
    compatible_lockfile: Option<&crate::lockfile::CompatibleLockfile>,
) -> ScenarioResult {
    if matches!(frozen_lockfile, FrozenLockfileMode::No) {
        console::verbose("scenario: Cold (lockfile disabled)");
        return ScenarioResult::cold();
    }

    if !lockfile_path.is_file() && compatible_lockfile.is_none() {
        console::verbose("scenario: Cold (no lockfile)");
        return ScenarioResult::cold();
    }

    let existing = match read_existing_lockfile(lockfile_path, compatible_lockfile, config) {
        Ok(lockfile) => lockfile,
        Err(error) => {
            console::warn(&format!(
                "scenario: Cold (compatible lockfile import failed: {})",
                error
            ));
            console::verbose("scenario: Cold (lockfile unreadable)");
            return ScenarioResult::cold();
        }
    };

    if !lockfile::root_specs_match(&existing, required_root, optional_root) {
        console::verbose("scenario: Cold (lockfile doesn't match manifest)");
        return ScenarioResult::cold();
    }

    let graph = lockfile::to_graph(&existing);
    let integrity_state = match build_project_integrity_state(project, &graph) {
        Ok(state) => state,
        Err(error) => {
            console::warn(&format!(
                "scenario: Cold (failed to compute install integrity state: {})",
                error
            ));
            return ScenarioResult::cold();
        }
    };

    if !force
        && check_integrity_file(project, &integrity_state)
        && check_project_layout_state(config, project, workspace, &graph, true)
    {
        console::verbose("scenario: Hot (lockfile + node_modules valid)");
        return ScenarioResult {
            scenario: InstallScenario::Hot,
            cache_check: None,
            graph: Some(graph),
            integrity_state: Some(integrity_state),
        };
    }

    if !force && check_integrity_file(project, &integrity_state) {
        console::verbose(
            "scenario: WarmLinkOnly/WarmPartialCache (integrity matched but install layout was stale)",
        );
    }

    let cache_check = check_store_cache(config, &graph);
    let missing_count = cache_check.missing.len();
    let total_count = graph.packages.len();

    if missing_count > 0
        && cache_check
            .missing
            .iter()
            .any(|package| package.tarball.is_empty())
    {
        console::verbose(
            "scenario: Cold (imported lockfile is missing tarball data for uncached packages)",
        );
        return ScenarioResult::cold();
    }

    if missing_count == 0 {
        console::verbose(&format!(
            "scenario: WarmLinkOnly ({} packages all cached)",
            total_count
        ));
        return ScenarioResult {
            scenario: InstallScenario::WarmLinkOnly,
            cache_check: Some(cache_check),
            graph: Some(graph),
            integrity_state: Some(integrity_state),
        };
    }

    console::verbose(&format!(
        "scenario: WarmPartialCache ({}/{} packages cached, {} missing)",
        total_count - missing_count,
        total_count,
        missing_count
    ));

    ScenarioResult {
        scenario: InstallScenario::WarmPartialCache,
        cache_check: Some(cache_check),
        graph: Some(graph),
        integrity_state: Some(integrity_state),
    }
}

fn read_existing_lockfile(
    lockfile_path: &Path,
    compatible_lockfile: Option<&crate::lockfile::CompatibleLockfile>,
    config: &SnpmConfig,
) -> crate::Result<crate::lockfile::Lockfile> {
    if lockfile_path.is_file() {
        return lockfile::read(lockfile_path);
    }

    let source = compatible_lockfile.ok_or_else(|| crate::SnpmError::Lockfile {
        path: lockfile_path.to_path_buf(),
        reason: "no lockfile was found".into(),
    })?;

    lockfile::read_compatible_lockfile(source, config)
}
