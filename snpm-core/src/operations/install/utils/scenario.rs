use super::integrity::{build_project_integrity_state, check_integrity_file};
use super::layout_state::check_project_layout_state;
use super::store::check_store_cache;
use super::types::{InstallScenario, ScenarioResult};
use super::{load_graph_snapshot, load_project_install_state};
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

    if let Some(source_path) = lockfile_source_path(lockfile_path, compatible_lockfile)
        && let Some(state) = load_project_install_state(
            config,
            project,
            workspace,
            &source_path,
            required_root,
            optional_root,
            true,
        )
    {
        if !state.root_specs_matches {
            console::verbose("scenario: Cold (graph snapshot root specs mismatch)");
            return ScenarioResult {
                scenario: InstallScenario::Cold,
                cache_check: None,
                graph: Some(state.graph),
                integrity_state: None,
            };
        }

        console::verbose(&format!(
            "scenario input: using graph snapshot from {}",
            source_path.display()
        ));
        return detect_from_graph(
            config,
            project,
            workspace,
            state.graph,
            force,
            Some(state.layout_valid),
        );
    }

    if let Some(source_path) = lockfile_source_path(lockfile_path, compatible_lockfile)
        && let Some(snapshot) = load_graph_snapshot(&project.root, &source_path)
    {
        if !snapshot.matches_root_specs(required_root, optional_root) {
            console::verbose("scenario: Cold (graph snapshot root specs mismatch)");
            return ScenarioResult {
                scenario: InstallScenario::Cold,
                cache_check: None,
                graph: Some(snapshot.graph),
                integrity_state: None,
            };
        }

        console::verbose(&format!(
            "scenario input: using graph snapshot from {}",
            source_path.display()
        ));
        return detect_from_graph(config, project, workspace, snapshot.graph, force, None);
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
    let graph = lockfile::to_graph(&existing);

    if !lockfile::root_specs_match(&existing, required_root, optional_root) {
        console::verbose("scenario: Cold (lockfile doesn't match manifest)");
        return ScenarioResult {
            scenario: InstallScenario::Cold,
            cache_check: None,
            graph: Some(graph),
            integrity_state: None,
        };
    }

    detect_from_graph(config, project, workspace, graph, force, None)
}

fn detect_from_graph(
    config: &SnpmConfig,
    project: &Project,
    workspace: Option<&crate::Workspace>,
    graph: crate::resolve::ResolutionGraph,
    force: bool,
    cached_layout_valid: Option<bool>,
) -> ScenarioResult {
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

    let layout_valid = cached_layout_valid
        .unwrap_or_else(|| check_project_layout_state(config, project, workspace, &graph, true));

    if !force && check_integrity_file(project, &integrity_state) && layout_valid {
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

#[cfg(test)]
mod tests {
    use super::detect_install_scenario;
    use crate::Project;
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
    use crate::operations::install::utils::{
        FrozenLockfileMode, InstallScenario, write_graph_snapshot,
    };
    use crate::project::Manifest;
    use crate::resolve::{
        PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency,
    };

    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    fn make_config() -> SnpmConfig {
        SnpmConfig {
            cache_dir: PathBuf::from("/tmp/cache"),
            data_dir: PathBuf::from("/tmp/data"),
            allow_scripts: BTreeSet::new(),
            disable_global_virtual_store_for_packages: BTreeSet::new(),
            min_package_age_days: None,
            min_package_cache_age_days: None,
            default_registry: "https://registry.npmjs.org".to_string(),
            scoped_registries: BTreeMap::new(),
            registry_auth: BTreeMap::new(),
            default_registry_auth_token: None,
            default_registry_auth_scheme: AuthScheme::Bearer,
            registry_auth_schemes: BTreeMap::new(),
            hoisting: HoistingMode::SingleVersion,
            link_backend: LinkBackend::Auto,
            strict_peers: false,
            frozen_lockfile_default: false,
            always_auth: false,
            registry_concurrency: 64,
            verbose: false,
            log_file: None,
        }
    }

    fn make_project(root: &Path) -> Project {
        Project {
            root: root.to_path_buf(),
            manifest_path: root.join("package.json"),
            manifest: Manifest {
                name: Some("app".to_string()),
                version: Some("1.0.0".to_string()),
                private: false,
                dependencies: BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
                dev_dependencies: BTreeMap::new(),
                optional_dependencies: BTreeMap::new(),
                scripts: BTreeMap::new(),
                resolutions: BTreeMap::new(),
                files: None,
                bin: None,
                main: None,
                pnpm: None,
                snpm: None,
                workspaces: None,
            },
        }
    }

    fn make_graph() -> ResolutionGraph {
        let id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };

        ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::from([(
                    "dep".to_string(),
                    RootDependency {
                        requested: "^1.0.0".to_string(),
                        resolved: id.clone(),
                    },
                )]),
            },
            packages: BTreeMap::from([(
                id.clone(),
                ResolvedPackage {
                    id,
                    tarball: "https://example.com/dep.tgz".to_string(),
                    integrity: None,
                    dependencies: BTreeMap::new(),
                    peer_dependencies: BTreeMap::new(),
                    bundled_dependencies: None,
                    has_bin: false,
                    bin: None,
                },
            )]),
        }
    }

    #[test]
    fn detect_install_scenario_returns_cold_with_snapshot_graph_when_root_specs_change() {
        let dir = tempdir().unwrap();
        let project = make_project(dir.path());
        fs::write(project.manifest_path.clone(), "{}").unwrap();

        let lockfile_path = dir.path().join("snpm-lock.yaml");
        fs::write(
            &lockfile_path,
            "version: 1\nroot:\n  dependencies: {}\npackages: {}\n",
        )
        .unwrap();

        write_graph_snapshot(
            dir.path(),
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
            &BTreeMap::new(),
            &make_graph(),
        )
        .unwrap();

        let result = detect_install_scenario(
            &project,
            None,
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^2.0.0".to_string())]),
            &BTreeMap::new(),
            &make_config(),
            FrozenLockfileMode::Prefer,
            false,
            None,
        );

        assert_eq!(result.scenario, InstallScenario::Cold);
        assert!(result.cache_check.is_none());
        assert!(result.integrity_state.is_none());
        assert_eq!(
            result
                .graph
                .as_ref()
                .and_then(|graph| graph.root.dependencies.get("dep"))
                .map(|dep| dep.resolved.version.as_str()),
            Some("1.0.0")
        );
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
