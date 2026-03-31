use crate::resolve::{PackageId, ResolutionGraph};

use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(super) struct PackageParent {
    pub parent: PackageId,
    pub via: String,
}

#[derive(Debug, Clone)]
pub(super) struct RootParent {
    pub name: String,
    pub requested: String,
}

#[derive(Default)]
pub(super) struct ReverseIndex {
    pub package_parents: BTreeMap<PackageId, Vec<PackageParent>>,
    pub root_parents: BTreeMap<PackageId, Vec<RootParent>>,
}

pub(super) fn build_reverse_index(graph: &ResolutionGraph) -> ReverseIndex {
    let mut index = ReverseIndex::default();

    for package in graph.packages.values() {
        for (dep_name, dep_id) in &package.dependencies {
            index
                .package_parents
                .entry(dep_id.clone())
                .or_default()
                .push(PackageParent {
                    parent: package.id.clone(),
                    via: dep_name.clone(),
                });
        }
    }

    for (root_name, root_dep) in &graph.root.dependencies {
        index
            .root_parents
            .entry(root_dep.resolved.clone())
            .or_default()
            .push(RootParent {
                name: root_name.clone(),
                requested: root_dep.requested.clone(),
            });
    }

    for parents in index.package_parents.values_mut() {
        parents.sort_by(|a, b| a.parent.cmp(&b.parent).then_with(|| a.via.cmp(&b.via)));
    }

    for roots in index.root_parents.values_mut() {
        roots.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| a.requested.cmp(&b.requested))
        });
    }

    index
}
