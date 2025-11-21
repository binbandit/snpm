use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency};
use crate::{Result, SnpmError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct LockRootDependency {
    pub requested: String,
    pub version: String,
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
    pub integrity: Option<String>,
    pub dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Lockfile {
    pub version: u32,
    pub root: LockRoot,
    pub packages: BTreeMap<String, LockPackage>,
}

pub fn write(path: &Path, graph: &ResolutionGraph) -> Result<()> {
    let mut root_deps = BTreeMap::new();

    for (name, dep) in graph.root.dependencies.iter() {
        let entry = LockRootDependency {
            requested: dep.requested.clone(),
            version: dep.resolved.version.clone(),
        };
        root_deps.insert(name.clone(), entry);
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
        };

        let key = format!("{}@{}", package.id.name, package.id.version);
        packages.insert(key, lock_pkg);
    }

    let lockfile = Lockfile {
        version: 1,
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
        let id = PackageId {
            name: name.clone(),
            version: dep.version.clone(),
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

fn split_dep_key(key: &str) -> Option<(String, String)> {
    if let Some(idx) = key.rfind('@') {
        let (name, version_part) = key.split_at(idx);
        let version = version_part.trim_start_matches('@').to_string();
        Some((name.to_string(), version))
    } else {
        None
    }
}
