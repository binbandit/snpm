use super::validate_lockfile_matches_manifest;
use crate::lockfile;
use crate::operations::install::utils::FrozenLockfileMode;

use std::collections::BTreeMap;
use std::path::PathBuf;

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
        &PathBuf::from("snpm-lock.yaml"),
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
        &PathBuf::from("snpm-lock.yaml"),
        InstallScenario::WarmLinkOnly,
        Some(lockfile),
        &required,
        &BTreeMap::new(),
    )
    .expect("frozen lockfile should not fail in prefer mode");

    assert_eq!(scenario, InstallScenario::WarmLinkOnly);
}
