use crate::resolve::ResolutionGraph;
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
