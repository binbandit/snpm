use super::integrity::{build_project_integrity_state, check_integrity_file};
use super::store::check_store_cache;
use super::types::{InstallScenario, ScenarioResult};
use crate::console;
use crate::{Project, SnpmConfig, lockfile};
use std::collections::BTreeMap;
use std::path::Path;

pub fn detect_install_scenario(
    project: &Project,
    lockfile_path: &Path,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
    config: &SnpmConfig,
    force: bool,
) -> ScenarioResult {
    if !lockfile_path.is_file() {
        console::verbose("scenario: Cold (no lockfile)");
        return ScenarioResult::cold();
    }

    let existing = match lockfile::read(lockfile_path) {
        Ok(lockfile) => lockfile,
        Err(_) => {
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

    if !force && check_integrity_file(project, &integrity_state) {
        console::verbose("scenario: Hot (lockfile + node_modules valid)");
        return ScenarioResult {
            scenario: InstallScenario::Hot,
            cache_check: None,
            graph: Some(graph),
            integrity_state: Some(integrity_state),
        };
    }

    let cache_check = check_store_cache(config, &graph);
    let missing_count = cache_check.missing.len();
    let total_count = graph.packages.len();

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
