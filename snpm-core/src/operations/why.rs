use crate::lockfile;
use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Project, Result, SnpmError, Workspace};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy)]
pub struct WhyOptions {
    pub depth: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct WhyResult {
    pub matches: Vec<WhyPackageMatch>,
}

#[derive(Debug, Serialize)]
pub struct WhyPackageMatch {
    pub name: String,
    pub version: String,
    pub paths: Vec<WhyPath>,
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct WhyPath {
    pub hops: Vec<WhyHop>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WhyHop {
    Package {
        name: String,
        version: String,
        via: String,
    },
    Root {
        name: String,
        requested: String,
    },
}

#[derive(Debug, Clone)]
struct PackageParent {
    parent: PackageId,
    via: String,
}

#[derive(Debug, Clone)]
struct RootParent {
    name: String,
    requested: String,
}

#[derive(Default)]
struct ReverseIndex {
    package_parents: BTreeMap<PackageId, Vec<PackageParent>>,
    root_parents: BTreeMap<PackageId, Vec<RootParent>>,
}

pub fn why(project: &Project, patterns: &[String], options: WhyOptions) -> Result<WhyResult> {
    let workspace = Workspace::discover(&project.root)?;
    let lockfile_path = workspace
        .as_ref()
        .map(|w| w.root.join("snpm-lock.yaml"))
        .unwrap_or_else(|| project.root.join("snpm-lock.yaml"));

    if !lockfile_path.is_file() {
        return Err(SnpmError::Lockfile {
            path: lockfile_path,
            reason: "snpm-lock.yaml is missing. Run `snpm install` first.".into(),
        });
    }

    let lock = lockfile::read(&lockfile_path)?;
    let graph = lockfile::to_graph(&lock);
    let index = build_reverse_index(&graph);

    let mut targets: Vec<PackageId> = graph
        .packages
        .keys()
        .filter(|id| {
            patterns
                .iter()
                .any(|pattern| matches_pattern(&id.name, pattern))
        })
        .cloned()
        .collect();

    targets.sort();

    let max_depth = options.depth.unwrap_or(usize::MAX);
    let mut matches = Vec::with_capacity(targets.len());

    for target in targets {
        let mut paths = Vec::new();
        let mut visited = BTreeSet::new();
        visited.insert(target.clone());

        walk_paths(
            &target,
            &index,
            max_depth,
            &mut visited,
            Vec::new(),
            0,
            &mut paths,
        );

        dedupe_paths(&mut paths);

        matches.push(WhyPackageMatch {
            name: target.name,
            version: target.version,
            paths,
        });
    }

    Ok(WhyResult { matches })
}

fn build_reverse_index(graph: &ResolutionGraph) -> ReverseIndex {
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

fn walk_paths(
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
            let mut emitted = false;

            if let Some(roots) = index.root_parents.get(&parent.parent) {
                for root in roots {
                    emitted = true;
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

            if !emitted {
                out.push(WhyPath {
                    hops: parent_hops,
                    truncated: true,
                });
            }

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

fn dedupe_paths(paths: &mut Vec<WhyPath>) {
    paths.sort();
    paths.dedup();
}

fn matches_pattern(value: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if !pattern.contains('*') {
        return value == pattern;
    }

    let starts_anchored = !pattern.starts_with('*');
    let ends_anchored = !pattern.ends_with('*');
    let parts: Vec<&str> = pattern.split('*').filter(|part| !part.is_empty()).collect();

    if parts.is_empty() {
        return true;
    }

    let mut cursor = 0usize;

    for (idx, part) in parts.iter().enumerate() {
        if idx == 0 && starts_anchored {
            if !value[cursor..].starts_with(part) {
                return false;
            }
            cursor += part.len();
            continue;
        }

        let Some(found) = value[cursor..].find(part) else {
            return false;
        };

        cursor += found + part.len();
    }

    if ends_anchored {
        if let Some(last) = parts.last() {
            value.ends_with(last)
        } else {
            true
        }
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{ResolutionRoot, ResolvedPackage, RootDependency};

    fn package_id(name: &str, version: &str) -> PackageId {
        PackageId {
            name: name.to_string(),
            version: version.to_string(),
        }
    }

    fn graph_fixture() -> ResolutionGraph {
        let target = package_id("target", "1.0.0");
        let mid = package_id("mid", "1.0.0");
        let top = package_id("top", "1.0.0");

        let mut packages = BTreeMap::new();

        packages.insert(
            target.clone(),
            ResolvedPackage {
                id: target.clone(),
                tarball: String::new(),
                integrity: None,
                dependencies: BTreeMap::new(),
                peer_dependencies: BTreeMap::new(),
                bundled_dependencies: None,
                has_bin: false,
            },
        );

        packages.insert(
            mid.clone(),
            ResolvedPackage {
                id: mid.clone(),
                tarball: String::new(),
                integrity: None,
                dependencies: BTreeMap::from([("target".to_string(), target.clone())]),
                peer_dependencies: BTreeMap::new(),
                bundled_dependencies: None,
                has_bin: false,
            },
        );

        packages.insert(
            top.clone(),
            ResolvedPackage {
                id: top.clone(),
                tarball: String::new(),
                integrity: None,
                dependencies: BTreeMap::from([("mid".to_string(), mid.clone())]),
                peer_dependencies: BTreeMap::new(),
                bundled_dependencies: None,
                has_bin: false,
            },
        );

        ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::from([(
                    "top".to_string(),
                    RootDependency {
                        requested: "^1.0.0".to_string(),
                        resolved: top,
                    },
                )]),
            },
            packages,
        }
    }

    #[test]
    fn collects_reverse_path_to_root() {
        let graph = graph_fixture();
        let index = build_reverse_index(&graph);
        let target = package_id("target", "1.0.0");
        let mut visited = BTreeSet::new();
        visited.insert(target.clone());

        let mut paths = Vec::new();
        walk_paths(
            &target,
            &index,
            usize::MAX,
            &mut visited,
            Vec::new(),
            0,
            &mut paths,
        );

        assert_eq!(paths.len(), 1);
        assert!(!paths[0].truncated);
        assert_eq!(paths[0].hops.len(), 3);
    }

    #[test]
    fn truncates_when_depth_reached() {
        let graph = graph_fixture();
        let index = build_reverse_index(&graph);
        let target = package_id("target", "1.0.0");
        let mut visited = BTreeSet::new();
        visited.insert(target.clone());

        let mut paths = Vec::new();
        walk_paths(&target, &index, 1, &mut visited, Vec::new(), 0, &mut paths);

        assert_eq!(paths.len(), 1);
        assert!(paths[0].truncated);
        assert_eq!(paths[0].hops.len(), 1);
    }

    #[test]
    fn wildcard_matching_supports_star() {
        assert!(matches_pattern("@types/react", "@types/*"));
        assert!(matches_pattern("left-pad", "*pad"));
        assert!(matches_pattern("left-pad", "left*"));
        assert!(!matches_pattern("react", "@types/*"));
    }
}
