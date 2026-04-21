use crate::lockfile;
use crate::operations::install::utils::FrozenLockfileMode;
use crate::resolve::ResolutionGraph;
use crate::{Result, SnpmConfig, Workspace};

use std::collections::BTreeMap;
use std::path::Path;

use super::super::super::utils::{
    CacheCheckResult, InstallScenario, IntegrityState, build_workspace_integrity_state,
    check_integrity_path, check_store_cache, check_workspace_layout_state, load_graph_snapshot,
    write_integrity_path,
};

pub(crate) struct WorkspaceScenarioArtifacts {
    pub(crate) scenario: InstallScenario,
    pub(crate) existing_lockfile: Option<lockfile::Lockfile>,
    pub(crate) graph: Option<ResolutionGraph>,
    pub(crate) cache_check: Option<CacheCheckResult>,
    pub(crate) lockfile_checked: bool,
}

impl WorkspaceScenarioArtifacts {
    fn cold(lockfile_checked: bool) -> Self {
        Self {
            scenario: InstallScenario::Cold,
            existing_lockfile: None,
            graph: None,
            cache_check: None,
            lockfile_checked,
        }
    }
}

pub(crate) fn detect_workspace_scenario_early(
    workspace: &Workspace,
    lockfile_path: &Path,
    compatible_lockfile: Option<&crate::lockfile::CompatibleLockfile>,
    config: &SnpmConfig,
    frozen_lockfile: FrozenLockfileMode,
    _strict_no_lockfile: bool,
    force: bool,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
) -> WorkspaceScenarioArtifacts {
    if matches!(
        frozen_lockfile,
        FrozenLockfileMode::No | FrozenLockfileMode::Fix
    ) {
        return WorkspaceScenarioArtifacts::cold(false);
    }

    if !lockfile_path.is_file() && compatible_lockfile.is_none() {
        return WorkspaceScenarioArtifacts::cold(false);
    }

    if let Some(source_path) = lockfile_source_path(lockfile_path, compatible_lockfile)
        && let Some(snapshot) = load_graph_snapshot(&workspace.root, &source_path)
        && snapshot.matches_root_specs(required_root, optional_root)
    {
        return detect_from_graph(workspace, config, snapshot.graph, force, None, true);
    }

    let existing = match read_existing_lockfile(lockfile_path, compatible_lockfile, config) {
        Ok(lockfile) => lockfile,
        Err(_) => return WorkspaceScenarioArtifacts::cold(false),
    };

    let graph = lockfile::to_graph(&existing);
    if !lockfile::root_specs_match(&existing, required_root, optional_root) {
        return WorkspaceScenarioArtifacts {
            scenario: InstallScenario::Cold,
            existing_lockfile: Some(existing),
            graph: Some(graph),
            cache_check: None,
            lockfile_checked: true,
        };
    }

    detect_from_graph(workspace, config, graph, force, Some(existing), true)
}

fn detect_from_graph(
    workspace: &Workspace,
    config: &SnpmConfig,
    graph: ResolutionGraph,
    force: bool,
    existing_lockfile: Option<lockfile::Lockfile>,
    lockfile_checked: bool,
) -> WorkspaceScenarioArtifacts {
    let integrity_state = match build_workspace_integrity_state(workspace, &graph) {
        Ok(state) => state,
        Err(_) => {
            return WorkspaceScenarioArtifacts {
                scenario: InstallScenario::Cold,
                existing_lockfile,
                graph: Some(graph),
                cache_check: None,
                lockfile_checked,
            };
        }
    };

    if !force
        && check_workspace_integrity(&workspace.root, &integrity_state)
        && check_workspace_layout_state(config, workspace, &graph, true)
    {
        return WorkspaceScenarioArtifacts {
            scenario: InstallScenario::Hot,
            existing_lockfile,
            graph: Some(graph),
            cache_check: None,
            lockfile_checked,
        };
    }

    let cache_check = check_store_cache(config, &graph);
    if cache_check
        .missing
        .iter()
        .any(|package| package.tarball.is_empty())
    {
        return WorkspaceScenarioArtifacts {
            scenario: InstallScenario::Cold,
            existing_lockfile,
            graph: Some(graph),
            cache_check: None,
            lockfile_checked,
        };
    }
    if cache_check.missing.is_empty() {
        return WorkspaceScenarioArtifacts {
            scenario: InstallScenario::WarmLinkOnly,
            existing_lockfile,
            graph: Some(graph),
            cache_check: Some(cache_check),
            lockfile_checked,
        };
    }

    WorkspaceScenarioArtifacts {
        scenario: InstallScenario::WarmPartialCache,
        existing_lockfile,
        graph: Some(graph),
        cache_check: Some(cache_check),
        lockfile_checked,
    }
}

pub(crate) fn validate_lockfile_matches_manifest(
    frozen_lockfile: FrozenLockfileMode,
    lockfile_path: &Path,
    strict_no_lockfile: bool,
    scenario: InstallScenario,
    lockfile: Option<lockfile::Lockfile>,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
    lockfile_checked: bool,
) -> Result<(InstallScenario, Option<lockfile::Lockfile>)> {
    if let Some(ref existing) = lockfile
        && !lockfile::root_specs_match(existing, required_root, optional_root)
    {
        if matches!(frozen_lockfile, FrozenLockfileMode::Frozen) {
            return Err(crate::SnpmError::Lockfile {
                path: lockfile_path.to_path_buf(),
                reason: "manifest dependencies do not match the existing lockfile when using frozen-lockfile"
                    .into(),
            });
        }

        return Ok((InstallScenario::Cold, lockfile));
    }

    if strict_no_lockfile
        && matches!(frozen_lockfile, FrozenLockfileMode::Frozen)
        && !lockfile_checked
    {
        return Err(crate::SnpmError::Lockfile {
            path: lockfile_path.to_path_buf(),
            reason: "frozen-lockfile requested but lockfile could not be read".into(),
        });
    }

    Ok((scenario, lockfile))
}

pub(crate) fn write_workspace_integrity(
    workspace_root: &Path,
    state: &IntegrityState,
) -> Result<()> {
    write_integrity_path(&workspace_root.join("node_modules"), state)
}

fn check_workspace_integrity(workspace_root: &Path, state: &IntegrityState) -> bool {
    check_integrity_path(&workspace_root.join("node_modules"), state)
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

fn lockfile_source_path(
    lockfile_path: &Path,
    compatible_lockfile: Option<&crate::lockfile::CompatibleLockfile>,
) -> Option<std::path::PathBuf> {
    if lockfile_path.is_file() {
        Some(lockfile_path.to_path_buf())
    } else {
        compatible_lockfile.map(|source| source.path.clone())
    }
}
