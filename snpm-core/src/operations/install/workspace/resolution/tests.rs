use super::{detect_workspace_scenario_early, validate_lockfile_matches_manifest};
use crate::lockfile;
use crate::operations::install::utils::{FrozenLockfileMode, write_graph_snapshot};
use crate::project::Manifest;
use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency};
use crate::workspace::types::WorkspaceConfig;
use crate::{SnpmConfig, Workspace, config::*};

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

use super::super::super::utils::InstallScenario;

#[test]
fn validate_lockfile_matches_returns_cold_on_mismatch() {
    let lockfile = lockfile::Lockfile {
        version: 1,
        root: lockfile::LockRoot {
            dependencies: BTreeMap::from([(
                "a".to_string(),
                lockfile::LockRootDependency {
                    requested: "^1.0.0".to_string(),
                    package: None,
                    version: Some("1.0.0".to_string()),
                    optional: false,
                },
            )]),
        },
        packages: BTreeMap::new(),
    };

    let required = BTreeMap::from([("b".to_string(), "^2.0.0".to_string())]);
    let (scenario, _) = validate_lockfile_matches_manifest(
        FrozenLockfileMode::Prefer,
        std::path::Path::new("snpm-lock.yaml"),
        false,
        InstallScenario::WarmLinkOnly,
        Some(lockfile),
        &required,
        &BTreeMap::new(),
        true,
    )
    .expect("frozen lockfile should not fail in prefer mode");

    assert_eq!(scenario, InstallScenario::Cold);
}

#[test]
fn validate_lockfile_matches_preserves_scenario_on_match() {
    let lockfile = lockfile::Lockfile {
        version: 1,
        root: lockfile::LockRoot {
            dependencies: BTreeMap::from([(
                "a".to_string(),
                lockfile::LockRootDependency {
                    requested: "^1.0.0".to_string(),
                    package: None,
                    version: Some("1.0.0".to_string()),
                    optional: false,
                },
            )]),
        },
        packages: BTreeMap::new(),
    };

    let required = BTreeMap::from([("a".to_string(), "^1.0.0".to_string())]);
    let (scenario, _) = validate_lockfile_matches_manifest(
        FrozenLockfileMode::Prefer,
        std::path::Path::new("snpm-lock.yaml"),
        false,
        InstallScenario::WarmLinkOnly,
        Some(lockfile),
        &required,
        &BTreeMap::new(),
        true,
    )
    .expect("frozen lockfile should not fail in prefer mode");

    assert_eq!(scenario, InstallScenario::WarmLinkOnly);
}

#[test]
fn validate_lockfile_matches_fix_mode_rewrites_to_cold_on_root_mismatch() {
    let lockfile = lockfile::Lockfile {
        version: 1,
        root: lockfile::LockRoot {
            dependencies: BTreeMap::from([(
                "a".to_string(),
                lockfile::LockRootDependency {
                    requested: "^1.0.0".to_string(),
                    package: None,
                    version: Some("1.0.0".to_string()),
                    optional: false,
                },
            )]),
        },
        packages: BTreeMap::new(),
    };

    let required = BTreeMap::from([("b".to_string(), "^2.0.0".to_string())]);
    let (scenario, _) = validate_lockfile_matches_manifest(
        FrozenLockfileMode::Fix,
        std::path::Path::new("snpm-lock.yaml"),
        false,
        InstallScenario::WarmLinkOnly,
        Some(lockfile),
        &required,
        &BTreeMap::new(),
        true,
    )
    .expect("fix mode should preserve lockfile validation and force cold resolution");

    assert_eq!(scenario, InstallScenario::Cold);
}

#[test]
fn validate_lockfile_matches_frozen_mode_with_strict_missing_lockfile_errors() {
    let error = validate_lockfile_matches_manifest(
        FrozenLockfileMode::Frozen,
        std::path::Path::new("snpm-lock.yaml"),
        true,
        InstallScenario::Cold,
        None,
        &BTreeMap::new(),
        &BTreeMap::new(),
        false,
    )
    .expect_err("frozen mode requires lockfile when strict flag is enabled");

    assert!(
        error
            .to_string()
            .contains("frozen-lockfile requested but lockfile could not be read")
    );
}

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

fn make_workspace(root: &Path) -> Workspace {
    Workspace {
        root: root.to_path_buf(),
        projects: vec![crate::Project {
            root: root.to_path_buf(),
            manifest_path: root.join("package.json"),
            manifest: Manifest {
                name: Some("workspace-root".to_string()),
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
        }],
        config: WorkspaceConfig {
            packages: Vec::new(),
            catalog: BTreeMap::new(),
            catalogs: BTreeMap::new(),
            only_built_dependencies: Vec::new(),
            ignored_built_dependencies: Vec::new(),
            disable_global_virtual_store_for_packages: None,
            hoisting: None,
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
fn detect_workspace_scenario_early_uses_graph_snapshot_before_parsing_lockfile() {
    let dir = tempdir().unwrap();
    let workspace = make_workspace(dir.path());
    let lockfile_path = dir.path().join("snpm-lock.yaml");
    std::fs::write(dir.path().join("package.json"), "{}").unwrap();
    std::fs::write(&lockfile_path, "definitely: [not valid yaml").unwrap();

    let required = BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]);
    write_graph_snapshot(
        dir.path(),
        &lockfile_path,
        &required,
        &BTreeMap::new(),
        &make_graph(),
    )
    .unwrap();

    let detected = detect_workspace_scenario_early(
        &workspace,
        &lockfile_path,
        None,
        &make_config(),
        FrozenLockfileMode::Prefer,
        false,
        false,
        &required,
        &BTreeMap::new(),
    );

    assert_eq!(detected.scenario, InstallScenario::WarmPartialCache);
    assert!(detected.graph.is_some());
    assert!(detected.existing_lockfile.is_none());
    assert!(detected.lockfile_checked);
}
