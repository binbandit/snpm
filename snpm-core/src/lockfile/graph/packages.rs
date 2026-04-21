use super::super::keys::split_dep_key;
use super::super::types::Lockfile;
use crate::resolve::{PackageId, ResolvedPackage};

use std::collections::BTreeMap;

pub(super) fn build_package_nodes(lockfile: &Lockfile) -> BTreeMap<PackageId, ResolvedPackage> {
    let mut packages = BTreeMap::new();

    for lock_pkg in lockfile.packages.values() {
        let id = PackageId {
            name: lock_pkg.name.clone(),
            version: lock_pkg.version.clone(),
        };

        packages.insert(
            id.clone(),
            ResolvedPackage {
                id,
                tarball: lock_pkg.tarball.clone(),
                integrity: lock_pkg.integrity.clone(),
                dependencies: BTreeMap::new(),
                peer_dependencies: BTreeMap::new(),
                bundled_dependencies: lock_pkg.bundled_dependencies.clone(),
                has_bin: lock_pkg.has_bin,
                bin: lock_pkg.bin.clone(),
            },
        );
    }

    packages
}

pub(super) fn populate_package_dependencies(
    lockfile: &Lockfile,
    packages: &mut BTreeMap<PackageId, ResolvedPackage>,
) {
    for lock_pkg in lockfile.packages.values() {
        let id = PackageId {
            name: lock_pkg.name.clone(),
            version: lock_pkg.version.clone(),
        };

        if let Some(package) = packages.get_mut(&id) {
            package.dependencies = lock_pkg
                .dependencies
                .iter()
                .filter_map(|(dep_name, dep_key)| {
                    let (name, version) = split_dep_key(dep_key)?;
                    Some((dep_name.clone(), PackageId { name, version }))
                })
                .collect();
        }
    }
}
