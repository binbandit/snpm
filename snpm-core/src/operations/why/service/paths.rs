use crate::operations::why::index::ReverseIndex;
use crate::operations::why::types::{WhyHop, WhyPackageMatch, WhyPath};
use crate::resolve::PackageId;

use std::collections::BTreeSet;

pub(super) fn build_match(
    target: &PackageId,
    index: &ReverseIndex,
    max_depth: usize,
) -> WhyPackageMatch {
    let mut paths = Vec::new();
    let mut visited = BTreeSet::new();
    visited.insert(target.clone());

    walk_paths(
        target,
        index,
        max_depth,
        &mut visited,
        Vec::new(),
        0,
        &mut paths,
    );
    dedupe_paths(&mut paths);

    WhyPackageMatch {
        name: target.name.clone(),
        version: target.version.clone(),
        paths,
    }
}

pub(super) fn walk_paths(
    current: &PackageId,
    index: &ReverseIndex,
    max_depth: usize,
    visited: &mut BTreeSet<PackageId>,
    hops: Vec<WhyHop>,
    depth: usize,
    out: &mut Vec<WhyPath>,
) {
    let root_parents = index.root_parents.get(current).cloned().unwrap_or_default();
    let package_parents = index
        .package_parents
        .get(current)
        .cloned()
        .unwrap_or_default();

    let mut advanced = false;

    for root in root_parents {
        advanced = true;
        let mut root_hops = hops.clone();
        root_hops.push(WhyHop::Root {
            name: root.name,
            requested: root.requested,
        });
        out.push(WhyPath {
            hops: root_hops,
            truncated: false,
        });
    }

    for parent in package_parents {
        if visited.contains(&parent.parent) {
            continue;
        }

        advanced = true;

        let mut parent_hops = hops.clone();
        parent_hops.push(WhyHop::Package {
            name: parent.parent.name.clone(),
            version: parent.parent.version.clone(),
            via: parent.via,
        });

        let next_depth = depth + 1;
        if next_depth >= max_depth {
            emit_truncated_or_root_paths(index, parent.parent.clone(), parent_hops, out);
            continue;
        }

        visited.insert(parent.parent.clone());
        walk_paths(
            &parent.parent,
            index,
            max_depth,
            visited,
            parent_hops,
            next_depth,
            out,
        );
        visited.remove(&parent.parent);
    }

    if !advanced {
        out.push(WhyPath {
            hops,
            truncated: false,
        });
    }
}

fn emit_truncated_or_root_paths(
    index: &ReverseIndex,
    parent: PackageId,
    parent_hops: Vec<WhyHop>,
    out: &mut Vec<WhyPath>,
) {
    let Some(roots) = index.root_parents.get(&parent) else {
        out.push(WhyPath {
            hops: parent_hops,
            truncated: true,
        });
        return;
    };

    for root in roots {
        let mut with_root = parent_hops.clone();
        with_root.push(WhyHop::Root {
            name: root.name.clone(),
            requested: root.requested.clone(),
        });
        out.push(WhyPath {
            hops: with_root,
            truncated: false,
        });
    }
}

fn dedupe_paths(paths: &mut Vec<WhyPath>) {
    paths.sort();
    paths.dedup();
}
