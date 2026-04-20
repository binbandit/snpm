use super::validate_lockfile_matches_manifest;
use crate::lockfile;
use crate::operations::install::utils::FrozenLockfileMode;

use std::collections::BTreeMap;

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
    )
    .expect_err("frozen mode requires lockfile when strict flag is enabled");

    assert!(error.to_string().contains("frozen-lockfile requested but lockfile could not be read"));
}
