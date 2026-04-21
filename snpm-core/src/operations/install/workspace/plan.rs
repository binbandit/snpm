use crate::lockfile;
use crate::operations::install::utils::{CacheCheckResult, FrozenLockfileMode};
use crate::resolve::ResolutionGraph;
use crate::{Result, SnpmConfig, Workspace};
use std::collections::BTreeMap;

use super::super::utils::InstallScenario;
use super::resolution::{
    WorkspaceScenarioArtifacts, detect_workspace_scenario_early, validate_lockfile_matches_manifest,
};
use super::setup::{WorkspaceInstallSetup, build_root_protocols, prepare_workspace_install};

pub(super) struct WorkspaceInstallPlan {
    pub(super) setup: WorkspaceInstallSetup,
    pub(super) scenario: InstallScenario,
    pub(super) scenario_graph: Option<ResolutionGraph>,
    pub(super) scenario_cache_check: Option<CacheCheckResult>,
}

fn pinned_workspace_root_dependencies_for_fix(
    setup: &WorkspaceInstallSetup,
    existing_lockfile: Option<&lockfile::Lockfile>,
) -> BTreeMap<String, String> {
    let mut root_dependencies = setup.root_dependencies.clone();
    let Some(existing) = existing_lockfile else {
        return root_dependencies;
    };

    for (name, requested) in &setup.root_specs.required {
        let Some(dep) = existing.root.dependencies.get(name) else {
            continue;
        };

        if dep.optional || dep.version.is_none() || dep.requested != *requested {
            continue;
        }

        if let Some(version) = dep.version.as_ref() {
            root_dependencies.insert(name.clone(), version.clone());
        }
    }

    for (name, requested) in &setup.root_specs.optional {
        let Some(dep) = existing.root.dependencies.get(name) else {
            continue;
        };

        if dep.requested != *requested || dep.version.is_none() {
            continue;
        }

        if let Some(version) = dep.version.as_ref() {
            root_dependencies.insert(name.clone(), version.clone());
        }
    }

    root_dependencies
}

pub(super) fn plan_workspace_install(
    config: &SnpmConfig,
    workspace: &Workspace,
    include_dev: bool,
    frozen_lockfile: FrozenLockfileMode,
    strict_no_lockfile: bool,
    force: bool,
) -> Result<WorkspaceInstallPlan> {
    let mut setup =
        prepare_workspace_install(workspace, include_dev, frozen_lockfile, strict_no_lockfile)?;
    let lockfile_source_path = setup.lockfile_source_path();
    let mut scenario_artifacts = if matches!(frozen_lockfile, FrozenLockfileMode::Fix) {
        let existing_lockfile = read_lockfile_for_fix(&setup, config).ok();
        WorkspaceScenarioArtifacts {
            scenario: InstallScenario::Cold,
            existing_lockfile: existing_lockfile.clone(),
            graph: existing_lockfile.as_ref().map(lockfile::to_graph),
            cache_check: None,
            lockfile_checked: existing_lockfile.is_some(),
        }
    } else {
        detect_workspace_scenario_early(
            workspace,
            &setup.lockfile_path,
            setup.compatible_lockfile.as_ref(),
            config,
            frozen_lockfile,
            strict_no_lockfile,
            force,
            &setup.root_specs.required,
            &setup.root_specs.optional,
        )
    };

    let (scenario, existing_lockfile) = validate_lockfile_matches_manifest(
        frozen_lockfile,
        &lockfile_source_path,
        strict_no_lockfile,
        scenario_artifacts.scenario,
        scenario_artifacts.existing_lockfile.take(),
        &setup.root_specs.required,
        &setup.root_specs.optional,
        scenario_artifacts.lockfile_checked,
    )?;
    scenario_artifacts.scenario = scenario;
    scenario_artifacts.existing_lockfile = existing_lockfile;

    if matches!(frozen_lockfile, FrozenLockfileMode::Fix) {
        setup.root_dependencies = pinned_workspace_root_dependencies_for_fix(
            &setup,
            scenario_artifacts.existing_lockfile.as_ref(),
        );
        setup.root_protocols = build_root_protocols(&setup.root_dependencies);
    }

    Ok(WorkspaceInstallPlan {
        setup,
        scenario: scenario_artifacts.scenario,
        scenario_graph: scenario_artifacts.graph,
        scenario_cache_check: scenario_artifacts.cache_check,
    })
}

fn read_lockfile_for_fix(
    setup: &WorkspaceInstallSetup,
    config: &SnpmConfig,
) -> crate::Result<crate::lockfile::Lockfile> {
    if setup.lockfile_path.is_file() {
        return crate::lockfile::read(&setup.lockfile_path);
    }

    let source = setup
        .compatible_lockfile
        .as_ref()
        .ok_or_else(|| crate::SnpmError::Lockfile {
            path: setup.lockfile_path.clone(),
            reason: "no lockfile was found".into(),
        })?;

    crate::lockfile::read_compatible_lockfile(source, config)
}

#[cfg(test)]
mod tests {
    use crate::lockfile::{LockRoot, LockRootDependency, Lockfile};
    use crate::operations::install::manifest::RootSpecSet;
    use crate::operations::install::utils::FrozenLockfileMode;
    use crate::operations::install::utils::InstallScenario;
    use crate::{Workspace, config::*};
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::Write;
    use std::path::Path;
    use tempfile::tempdir;

    fn make_workspace_config() -> SnpmConfig {
        SnpmConfig {
            cache_dir: Path::new("/tmp/cache").to_path_buf(),
            data_dir: Path::new("/tmp/data").to_path_buf(),
            allow_scripts: BTreeSet::new(),
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

    fn write_manifest(path: &Path, contents: &str) {
        let mut manifest = std::fs::File::create(path).unwrap();
        manifest.write_all(contents.as_bytes()).unwrap();
    }

    #[test]
    fn plan_workspace_install_fix_mode_loads_existing_lockfile_and_pins_matching_versions() {
        let dir = tempdir().unwrap();
        write_manifest(
            &dir.path().join("package.json"),
            r#"{
  "name": "workspace-root",
  "version": "1.0.0",
  "dependencies": {
    "foo": "^1.0.0"
  },
  "optionalDependencies": {
    "opt": "^3.0.0"
  }
}"#,
        );
        write_manifest(&dir.path().join("snpm-workspace.yaml"), "packages: []\n");

        let lockfile = Lockfile {
            version: 1,
            root: LockRoot {
                dependencies: BTreeMap::from([
                    (
                        "foo".to_string(),
                        LockRootDependency {
                            requested: "^1.0.0".to_string(),
                            package: None,
                            version: Some("1.2.3".to_string()),
                            optional: false,
                        },
                    ),
                    (
                        "opt".to_string(),
                        LockRootDependency {
                            requested: "^3.0.0".to_string(),
                            package: None,
                            version: Some("3.4.5".to_string()),
                            optional: true,
                        },
                    ),
                ]),
            },
            packages: std::collections::BTreeMap::new(),
        };
        write_lockfile(dir.path(), lockfile);

        let workspace = Workspace::discover(dir.path()).unwrap().unwrap();
        let plan = super::plan_workspace_install(
            &make_workspace_config(),
            &workspace,
            true,
            FrozenLockfileMode::Fix,
            false,
            false,
        )
        .unwrap();

        assert_eq!(plan.scenario, InstallScenario::Cold);
        assert!(plan.scenario_graph.is_some());
        assert_eq!(
            plan.setup.root_dependencies.get("foo"),
            Some(&"1.2.3".to_string())
        );
        assert_eq!(
            plan.setup.root_dependencies.get("opt"),
            Some(&"3.4.5".to_string())
        );
    }

    #[test]
    fn pinned_workspace_root_dependencies_for_fix_only_rewrites_matching_locked_deps() {
        let setup = super::super::setup::WorkspaceInstallSetup {
            lockfile_path: Path::new("/tmp/snpm-lock.yaml").to_path_buf(),
            compatible_lockfile: None,
            overrides: BTreeMap::new(),
            root_specs: RootSpecSet {
                required: BTreeMap::from([("left".to_string(), "^1.0.0".to_string())]),
                optional: BTreeMap::from([("right".to_string(), "^2.0.0".to_string())]),
            },
            root_dependencies: BTreeMap::from([
                ("left".to_string(), "^1.0.0".to_string()),
                ("right".to_string(), "^2.0.0".to_string()),
            ]),
            root_protocols: BTreeMap::new(),
            optional_root_names: BTreeSet::new(),
        };

        let lockfile = Lockfile {
            version: 1,
            root: LockRoot {
                dependencies: BTreeMap::from([
                    (
                        "left".to_string(),
                        LockRootDependency {
                            requested: "^1.0.0".to_string(),
                            package: None,
                            version: Some("1.2.3".to_string()),
                            optional: false,
                        },
                    ),
                    (
                        "right".to_string(),
                        LockRootDependency {
                            requested: "^9.0.0".to_string(),
                            package: None,
                            version: Some("9.9.9".to_string()),
                            optional: true,
                        },
                    ),
                ]),
            },
            packages: std::collections::BTreeMap::new(),
        };

        let rewritten = super::pinned_workspace_root_dependencies_for_fix(&setup, Some(&lockfile));

        let expected = BTreeMap::from([
            ("left".to_string(), "1.2.3".to_string()),
            ("right".to_string(), "^2.0.0".to_string()),
        ]);
        assert_eq!(rewritten, expected);
    }

    #[test]
    fn plan_workspace_install_fix_mode_without_existing_lockfile_preserves_ranges() {
        let dir = tempdir().unwrap();
        write_manifest(
            &dir.path().join("package.json"),
            r#"{
  "name": "workspace-root",
  "version": "1.0.0",
  "dependencies": {
    "left": "^1.0.0"
  }
}"#,
        );
        write_manifest(&dir.path().join("snpm-workspace.yaml"), "packages: []\n");

        let workspace = Workspace::discover(dir.path()).unwrap().unwrap();
        let plan = super::plan_workspace_install(
            &make_workspace_config(),
            &workspace,
            true,
            FrozenLockfileMode::Fix,
            false,
            false,
        )
        .unwrap();

        assert_eq!(plan.scenario, InstallScenario::Cold);
        assert!(plan.scenario_graph.is_none());
        assert_eq!(
            plan.setup.root_dependencies.get("left"),
            Some(&"^1.0.0".to_string())
        );
    }

    #[test]
    fn read_lockfile_for_fix_requires_lockfile_or_compatible_source() {
        let dir = tempdir().unwrap();
        let setup = super::super::setup::WorkspaceInstallSetup {
            lockfile_path: dir.path().join("missing.yaml"),
            compatible_lockfile: None,
            overrides: BTreeMap::new(),
            root_specs: RootSpecSet {
                required: BTreeMap::new(),
                optional: BTreeMap::new(),
            },
            root_dependencies: BTreeMap::new(),
            root_protocols: BTreeMap::new(),
            optional_root_names: BTreeSet::new(),
        };
        let result = super::read_lockfile_for_fix(&setup, &make_workspace_config());

        assert!(result.is_err());
    }

    fn write_lockfile(path: &Path, lockfile: Lockfile) {
        let lockfile_path = path.join("snpm-lock.yaml");
        let data = serde_yaml::to_string(&lockfile).unwrap();
        std::fs::write(&lockfile_path, data).unwrap();
    }
}
