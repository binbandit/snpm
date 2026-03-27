use crate::registry::BundledDependencies;
use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency};
use crate::{Result, SnpmError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Current lockfile schema version.
const LOCKFILE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct LockRootDependency {
    pub requested: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LockRoot {
    pub dependencies: BTreeMap<String, LockRootDependency>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LockPackage {
    pub name: String,
    pub version: String,
    pub tarball: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrity: Option<String>,
    pub dependencies: BTreeMap<String, String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "bundledDependencies"
    )]
    pub bundled_dependencies: Option<BundledDependencies>,
    #[serde(default, skip_serializing_if = "is_false", rename = "hasBin")]
    pub has_bin: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Lockfile {
    pub version: u32,
    pub root: LockRoot,
    pub packages: BTreeMap<String, LockPackage>,
}

pub fn write(
    path: &Path,
    graph: &ResolutionGraph,
    optional_root_specs: &BTreeMap<String, String>,
) -> Result<()> {
    let mut root_deps = BTreeMap::new();

    for (name, dep) in graph.root.dependencies.iter() {
        let entry = LockRootDependency {
            requested: dep.requested.clone(),
            version: Some(dep.resolved.version.clone()),
            optional: optional_root_specs.contains_key(name),
        };
        root_deps.insert(name.clone(), entry);
    }

    for (name, requested) in optional_root_specs {
        root_deps.entry(name.clone()).or_insert(LockRootDependency {
            requested: requested.clone(),
            version: None,
            optional: true,
        });
    }

    let mut packages = BTreeMap::new();

    for package in graph.packages.values() {
        let mut deps = BTreeMap::new();

        for (dep_name, dep_id) in package.dependencies.iter() {
            let key = format!("{}@{}", dep_id.name, dep_id.version);
            deps.insert(dep_name.clone(), key);
        }

        let lock_pkg = LockPackage {
            name: package.id.name.clone(),
            version: package.id.version.clone(),
            tarball: package.tarball.clone(),
            integrity: package.integrity.clone(),
            dependencies: deps,
            bundled_dependencies: package.bundled_dependencies.clone(),
            has_bin: package.has_bin,
        };

        let key = format!("{}@{}", package.id.name, package.id.version);
        packages.insert(key, lock_pkg);
    }

    let lockfile = Lockfile {
        version: LOCKFILE_VERSION,
        root: LockRoot {
            dependencies: root_deps,
        },
        packages,
    };

    let data = serde_yaml::to_string(&lockfile).map_err(|source| SnpmError::LockfileWrite {
        path: path.to_path_buf(),
        source,
    })?;

    fs::write(path, data).map_err(|source| SnpmError::WriteFile {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(())
}

pub fn read(path: &Path) -> Result<Lockfile> {
    let data = fs::read_to_string(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let lockfile: Lockfile = serde_yaml::from_str(&data).map_err(|err| SnpmError::Lockfile {
        path: path.to_path_buf(),
        reason: err.to_string(),
    })?;

    if lockfile.version != LOCKFILE_VERSION {
        return Err(SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: format!(
                "unsupported lockfile version {} (expected {}), delete the lockfile and reinstall",
                lockfile.version, LOCKFILE_VERSION
            ),
        });
    }

    Ok(lockfile)
}

pub fn to_graph(lockfile: &Lockfile) -> ResolutionGraph {
    let mut packages = BTreeMap::new();

    for lock_pkg in lockfile.packages.values() {
        let id = PackageId {
            name: lock_pkg.name.clone(),
            version: lock_pkg.version.clone(),
        };

        let resolved = ResolvedPackage {
            id: id.clone(),
            tarball: lock_pkg.tarball.clone(),
            integrity: lock_pkg.integrity.clone(),
            dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: lock_pkg.bundled_dependencies.clone(),
            has_bin: lock_pkg.has_bin,
        };

        packages.insert(id, resolved);
    }

    for lock_pkg in lockfile.packages.values() {
        let id = PackageId {
            name: lock_pkg.name.clone(),
            version: lock_pkg.version.clone(),
        };

        if let Some(package) = packages.get_mut(&id) {
            let mut deps = BTreeMap::new();

            for (dep_name, dep_key) in lock_pkg.dependencies.iter() {
                if let Some((dep_pkg_name, dep_pkg_version)) = split_dep_key(dep_key) {
                    let dep_id = PackageId {
                        name: dep_pkg_name,
                        version: dep_pkg_version,
                    };
                    deps.insert(dep_name.clone(), dep_id);
                }
            }

            package.dependencies = deps;
        }
    }

    let mut root_dependencies = BTreeMap::new();

    for (name, dep) in lockfile.root.dependencies.iter() {
        let Some(version) = dep.version.as_ref() else {
            continue;
        };

        let id = PackageId {
            name: name.clone(),
            version: version.clone(),
        };
        let root_dep = RootDependency {
            requested: dep.requested.clone(),
            resolved: id,
        };
        root_dependencies.insert(name.clone(), root_dep);
    }

    let root = ResolutionRoot {
        dependencies: root_dependencies,
    };

    ResolutionGraph { root, packages }
}

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

fn split_dep_key(key: &str) -> Option<(String, String)> {
    if let Some(idx) = key.rfind('@') {
        let (name, version_part) = key.split_at(idx);
        let version = version_part.trim_start_matches('@').to_string();
        Some((name.to_string(), version))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                            version: Some("1.2.3".to_string()),
                            optional: false,
                        },
                    ),
                    (
                        "optional".to_string(),
                        LockRootDependency {
                            requested: "^2.0.0".to_string(),
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
    fn to_graph_skips_unresolved_optional_roots() {
        let lockfile = Lockfile {
            version: 1,
            root: LockRoot {
                dependencies: BTreeMap::from([
                    (
                        "required".to_string(),
                        LockRootDependency {
                            requested: "^1.0.0".to_string(),
                            version: Some("1.2.3".to_string()),
                            optional: false,
                        },
                    ),
                    (
                        "optional".to_string(),
                        LockRootDependency {
                            requested: "^2.0.0".to_string(),
                            version: None,
                            optional: true,
                        },
                    ),
                ]),
            },
            packages: BTreeMap::new(),
        };

        let graph = to_graph(&lockfile);

        assert!(graph.root.dependencies.contains_key("required"));
        assert!(!graph.root.dependencies.contains_key("optional"));
    }

    #[test]
    fn split_dep_key_simple() {
        let result = split_dep_key("lodash@4.17.21");
        assert_eq!(result, Some(("lodash".to_string(), "4.17.21".to_string())));
    }

    #[test]
    fn split_dep_key_scoped() {
        let result = split_dep_key("@types/node@18.0.0");
        assert_eq!(
            result,
            Some(("@types/node".to_string(), "18.0.0".to_string()))
        );
    }

    #[test]
    fn split_dep_key_no_at() {
        assert!(split_dep_key("lodash").is_none());
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

    #[test]
    fn to_graph_reconstructs_dependencies() {
        let lockfile = Lockfile {
            version: 1,
            root: LockRoot {
                dependencies: BTreeMap::from([(
                    "express".to_string(),
                    LockRootDependency {
                        requested: "^4.0.0".to_string(),
                        version: Some("4.18.2".to_string()),
                        optional: false,
                    },
                )]),
            },
            packages: BTreeMap::from([
                (
                    "express@4.18.2".to_string(),
                    LockPackage {
                        name: "express".to_string(),
                        version: "4.18.2".to_string(),
                        tarball: "https://registry.npmjs.org/express/-/express-4.18.2.tgz"
                            .to_string(),
                        integrity: Some("sha512-abc123".to_string()),
                        dependencies: BTreeMap::from([(
                            "body-parser".to_string(),
                            "body-parser@1.20.0".to_string(),
                        )]),
                        bundled_dependencies: None,
                        has_bin: true,
                    },
                ),
                (
                    "body-parser@1.20.0".to_string(),
                    LockPackage {
                        name: "body-parser".to_string(),
                        version: "1.20.0".to_string(),
                        tarball: "https://registry.npmjs.org/body-parser/-/body-parser-1.20.0.tgz"
                            .to_string(),
                        integrity: None,
                        dependencies: BTreeMap::new(),
                        bundled_dependencies: None,
                        has_bin: false,
                    },
                ),
            ]),
        };

        let graph = to_graph(&lockfile);

        assert!(graph.root.dependencies.contains_key("express"));
        assert_eq!(
            graph.root.dependencies["express"].resolved.version,
            "4.18.2"
        );

        let express_id = PackageId {
            name: "express".to_string(),
            version: "4.18.2".to_string(),
        };
        let express = graph.packages.get(&express_id).unwrap();
        assert!(express.has_bin);
        assert!(express.dependencies.contains_key("body-parser"));
        assert_eq!(express.dependencies["body-parser"].version, "1.20.0");
    }

    #[test]
    fn write_and_read_round_trip() {
        use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency};

        let id = PackageId {
            name: "test-pkg".to_string(),
            version: "1.0.0".to_string(),
        };
        let pkg = ResolvedPackage {
            id: id.clone(),
            tarball: "https://example.com/test-pkg-1.0.0.tgz".to_string(),
            integrity: Some("sha512-abc".to_string()),
            dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: None,
            has_bin: false,
        };

        let graph = ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::from([(
                    "test-pkg".to_string(),
                    RootDependency {
                        requested: "^1.0.0".to_string(),
                        resolved: id.clone(),
                    },
                )]),
            },
            packages: BTreeMap::from([(id, pkg)]),
        };

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snpm-lock.yaml");

        write(&path, &graph, &BTreeMap::new()).unwrap();
        let lockfile = read(&path).unwrap();

        assert_eq!(lockfile.version, 1);
        assert!(lockfile.root.dependencies.contains_key("test-pkg"));
        assert_eq!(
            lockfile.root.dependencies["test-pkg"].requested,
            "^1.0.0"
        );
        assert!(lockfile.packages.contains_key("test-pkg@1.0.0"));
        let pkg = &lockfile.packages["test-pkg@1.0.0"];
        assert_eq!(pkg.tarball, "https://example.com/test-pkg-1.0.0.tgz");
        assert_eq!(pkg.integrity.as_deref(), Some("sha512-abc"));
    }

    #[test]
    fn read_rejects_wrong_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("snpm-lock.yaml");

        let data = "version: 99\nroot:\n  dependencies: {}\npackages: {}\n";
        std::fs::write(&path, data).unwrap();

        let result = read(&path);
        assert!(result.is_err());
    }
}
