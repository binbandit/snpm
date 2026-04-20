use super::types::Lockfile;
use std::collections::BTreeMap;

pub fn root_specs_match(
    lockfile: &Lockfile,
    required: &BTreeMap<String, String>,
    optional: &BTreeMap<String, String>,
) -> bool {
    if lockfile.root.dependencies.len() != required.len() + optional.len() {
        return false;
    }

    for (name, requested) in required {
        let Some(dep) = lockfile.root.dependencies.get(name) else {
            return false;
        };

        if dep.requested != *requested || dep.version.is_none() || dep.optional {
            return false;
        }
    }

    for (name, requested) in optional {
        let Some(dep) = lockfile.root.dependencies.get(name) else {
            return false;
        };

        if dep.requested != *requested {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lockfile::{LockRoot, LockRootDependency};

    #[test]
    fn root_specs_match_accepts_unresolved_optional_roots() {
        let lockfile = Lockfile {
            version: 1,
            root: LockRoot {
                dependencies: BTreeMap::from([
                    (
                        "required".to_string(),
                        LockRootDependency {
                            requested: "^1.0.0".to_string(),
                            package: None,
                            version: Some("1.2.3".to_string()),
                            optional: false,
                        },
                    ),
                    (
                        "optional".to_string(),
                        LockRootDependency {
                            requested: "^2.0.0".to_string(),
                            package: None,
                            version: None,
                            optional: true,
                        },
                    ),
                ]),
            },
            packages: BTreeMap::new(),
        };

        let required = BTreeMap::from([("required".to_string(), "^1.0.0".to_string())]);
        let optional = BTreeMap::from([("optional".to_string(), "^2.0.0".to_string())]);

        assert!(root_specs_match(&lockfile, &required, &optional));
    }

    #[test]
    fn root_specs_match_rejects_different_count() {
        let lockfile = Lockfile {
            version: 1,
            root: LockRoot {
                dependencies: BTreeMap::from([(
                    "a".to_string(),
                    LockRootDependency {
                        requested: "^1.0.0".to_string(),
                        package: None,
                        version: Some("1.0.0".to_string()),
                        optional: false,
                    },
                )]),
            },
            packages: BTreeMap::new(),
        };

        let required = BTreeMap::from([
            ("a".to_string(), "^1.0.0".to_string()),
            ("b".to_string(), "^2.0.0".to_string()),
        ]);
        assert!(!root_specs_match(&lockfile, &required, &BTreeMap::new()));
    }

    #[test]
    fn root_specs_match_rejects_different_range() {
        let lockfile = Lockfile {
            version: 1,
            root: LockRoot {
                dependencies: BTreeMap::from([(
                    "a".to_string(),
                    LockRootDependency {
                        requested: "^1.0.0".to_string(),
                        package: None,
                        version: Some("1.0.0".to_string()),
                        optional: false,
                    },
                )]),
            },
            packages: BTreeMap::new(),
        };

        let required = BTreeMap::from([("a".to_string(), "^2.0.0".to_string())]);
        assert!(!root_specs_match(&lockfile, &required, &BTreeMap::new()));
    }

    #[test]
    fn root_specs_match_rejects_unresolved_required() {
        let lockfile = Lockfile {
            version: 1,
            root: LockRoot {
                dependencies: BTreeMap::from([(
                    "a".to_string(),
                    LockRootDependency {
                        requested: "^1.0.0".to_string(),
                        package: None,
                        version: None,
                        optional: false,
                    },
                )]),
            },
            packages: BTreeMap::new(),
        };

        let required = BTreeMap::from([("a".to_string(), "^1.0.0".to_string())]);
        assert!(!root_specs_match(&lockfile, &required, &BTreeMap::new()));
    }

    #[test]
    fn root_specs_match_rejects_missing_dep() {
        let lockfile = Lockfile {
            version: 1,
            root: LockRoot {
                dependencies: BTreeMap::new(),
            },
            packages: BTreeMap::new(),
        };

        let required = BTreeMap::from([("a".to_string(), "^1.0.0".to_string())]);
        assert!(!root_specs_match(&lockfile, &required, &BTreeMap::new()));
    }
}
