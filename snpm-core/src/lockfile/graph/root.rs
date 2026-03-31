use super::super::types::Lockfile;
use crate::resolve::{PackageId, RootDependency};

use std::collections::BTreeMap;

pub(super) fn build_root_dependencies(lockfile: &Lockfile) -> BTreeMap<String, RootDependency> {
    let mut root_dependencies = BTreeMap::new();

    for (name, dep) in &lockfile.root.dependencies {
        let Some(version) = dep.version.as_ref() else {
            continue;
        };

        root_dependencies.insert(
            name.clone(),
            RootDependency {
                requested: dep.requested.clone(),
                resolved: PackageId {
                    name: name.clone(),
                    version: version.clone(),
                },
            },
        );
    }

    root_dependencies
}
