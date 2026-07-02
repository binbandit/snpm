//! A bincode-safe mirror of `ResolutionGraph` for the install-state
//! snapshot.
//!
//! `ResolvedPackage` embeds `#[serde(untagged)]` enums (`BinField`,
//! `BundledDependencies`). bincode is not self-describing, so it cannot
//! deserialize untagged enums (`DeserializeAnyNotSupported`) — storing a
//! `ResolutionGraph` directly made the install-state file impossible to
//! read back, silently disabling the hot/warm fast path and forcing a
//! full re-link on every install.
//!
//! This mirror keeps the graph's structure (including `PackageId` map
//! keys, which bincode handles) but swaps the untagged enums for
//! ordinary tagged ones that round-trip cleanly.

use crate::project::BinField;
use crate::registry::BundledDependencies;
use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SnapshotGraph {
    root: BTreeMap<String, SnapshotRootDependency>,
    packages: BTreeMap<PackageId, SnapshotPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotRootDependency {
    requested: String,
    resolved: PackageId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotPackage {
    id: PackageId,
    tarball: String,
    integrity: Option<String>,
    dependencies: BTreeMap<String, PackageId>,
    peer_dependencies: BTreeMap<String, String>,
    bundled_dependencies: Option<SnapshotBundled>,
    has_bin: bool,
    bin: Option<SnapshotBin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum SnapshotBundled {
    List(Vec<String>),
    All(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum SnapshotBin {
    Single(String),
    Map(BTreeMap<String, String>),
}

impl From<&ResolutionGraph> for SnapshotGraph {
    fn from(graph: &ResolutionGraph) -> Self {
        Self {
            root: graph
                .root
                .dependencies
                .iter()
                .map(|(name, dep)| {
                    (
                        name.clone(),
                        SnapshotRootDependency {
                            requested: dep.requested.clone(),
                            resolved: dep.resolved.clone(),
                        },
                    )
                })
                .collect(),
            packages: graph
                .packages
                .iter()
                .map(|(id, package)| (id.clone(), SnapshotPackage::from(package)))
                .collect(),
        }
    }
}

impl From<SnapshotGraph> for ResolutionGraph {
    fn from(snapshot: SnapshotGraph) -> Self {
        Self {
            root: ResolutionRoot {
                dependencies: snapshot
                    .root
                    .into_iter()
                    .map(|(name, dep)| {
                        (
                            name,
                            RootDependency {
                                requested: dep.requested,
                                resolved: dep.resolved,
                            },
                        )
                    })
                    .collect(),
            },
            packages: snapshot
                .packages
                .into_iter()
                .map(|(id, package)| (id, package.into()))
                .collect(),
        }
    }
}

impl From<&ResolvedPackage> for SnapshotPackage {
    fn from(package: &ResolvedPackage) -> Self {
        Self {
            id: package.id.clone(),
            tarball: package.tarball.clone(),
            integrity: package.integrity.clone(),
            dependencies: package.dependencies.clone(),
            peer_dependencies: package.peer_dependencies.clone(),
            bundled_dependencies: package.bundled_dependencies.as_ref().map(
                |bundled| match bundled {
                    BundledDependencies::List(list) => SnapshotBundled::List(list.clone()),
                    BundledDependencies::All(all) => SnapshotBundled::All(*all),
                },
            ),
            has_bin: package.has_bin,
            bin: package.bin.as_ref().map(|bin| match bin {
                BinField::Single(value) => SnapshotBin::Single(value.clone()),
                BinField::Map(map) => SnapshotBin::Map(map.clone()),
            }),
        }
    }
}

impl From<SnapshotPackage> for ResolvedPackage {
    fn from(package: SnapshotPackage) -> Self {
        Self {
            id: package.id,
            tarball: package.tarball,
            integrity: package.integrity,
            dependencies: package.dependencies,
            peer_dependencies: package.peer_dependencies,
            bundled_dependencies: package.bundled_dependencies.map(|bundled| match bundled {
                SnapshotBundled::List(list) => BundledDependencies::List(list),
                SnapshotBundled::All(all) => BundledDependencies::All(all),
            }),
            has_bin: package.has_bin,
            bin: package.bin.map(|bin| match bin {
                SnapshotBin::Single(value) => BinField::Single(value),
                SnapshotBin::Map(map) => BinField::Map(map),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SnapshotGraph;
    use crate::project::BinField;
    use crate::registry::BundledDependencies;
    use crate::resolve::{
        PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency,
    };
    use std::collections::BTreeMap;

    #[test]
    fn snapshot_graph_bincode_round_trips_with_untagged_enums() {
        let id = PackageId {
            name: "tool".to_string(),
            version: "1.0.0".to_string(),
        };
        let graph = ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::from([(
                    "tool".to_string(),
                    RootDependency {
                        requested: "^1.0.0".to_string(),
                        resolved: id.clone(),
                    },
                )]),
            },
            packages: BTreeMap::from([(
                id.clone(),
                ResolvedPackage {
                    id: id.clone(),
                    tarball: "https://example.com/tool.tgz".to_string(),
                    integrity: Some("sha512-abc".to_string()),
                    dependencies: BTreeMap::new(),
                    peer_dependencies: BTreeMap::from([("react".to_string(), ">=17".to_string())]),
                    bundled_dependencies: Some(BundledDependencies::List(vec!["vendored".into()])),
                    has_bin: true,
                    bin: Some(BinField::Map(BTreeMap::from([(
                        "tool".to_string(),
                        "bin/tool.js".to_string(),
                    )]))),
                },
            )]),
        };

        // This is the exact operation that failed for the raw graph.
        let bytes = bincode::serialize(&SnapshotGraph::from(&graph)).unwrap();
        let restored: ResolutionGraph = bincode::deserialize::<SnapshotGraph>(&bytes)
            .unwrap()
            .into();

        let pkg = &restored.packages[&id];
        assert_eq!(pkg.tarball, "https://example.com/tool.tgz");
        assert_eq!(pkg.peer_dependencies["react"], ">=17");
        assert!(matches!(pkg.bin, Some(BinField::Map(_))));
        assert!(matches!(
            pkg.bundled_dependencies,
            Some(BundledDependencies::List(_))
        ));
        assert_eq!(restored.root.dependencies["tool"].resolved.version, "1.0.0");
    }
}
