use super::validate_lockfile_matches_manifest;
use crate::lockfile;

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
                    version: Some("1.0.0".to_string()),
                    optional: false,
                },
            )]),
        },
        packages: BTreeMap::new(),
    };

    let required = BTreeMap::from([("b".to_string(), "^2.0.0".to_string())]);
    let (scenario, _) = validate_lockfile_matches_manifest(
        InstallScenario::WarmLinkOnly,
        Some(lockfile),
        &required,
        &BTreeMap::new(),
    );

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
                    version: Some("1.0.0".to_string()),
                    optional: false,
                },
            )]),
        },
        packages: BTreeMap::new(),
    };

    let required = BTreeMap::from([("a".to_string(), "^1.0.0".to_string())]);
    let (scenario, _) = validate_lockfile_matches_manifest(
        InstallScenario::WarmLinkOnly,
        Some(lockfile),
        &required,
        &BTreeMap::new(),
    );

    assert_eq!(scenario, InstallScenario::WarmLinkOnly);
}
