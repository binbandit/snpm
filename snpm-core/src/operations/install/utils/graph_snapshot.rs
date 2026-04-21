use crate::resolve::ResolutionGraph;
use crate::{Result, SnpmError};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const GRAPH_SNAPSHOT_FILE: &str = ".snpm-graph-snapshot.bin";
const GRAPH_SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GraphSnapshot {
    version: u32,
    source: SnapshotSource,
    root_specs_hash: String,
    graph: ResolutionGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SnapshotSource {
    path: PathBuf,
    length: u64,
    modified_seconds: u64,
    modified_nanoseconds: u32,
}

pub(crate) struct LoadedGraphSnapshot {
    pub(crate) graph: ResolutionGraph,
    pub(crate) root_specs_hash: String,
}

pub(crate) fn write_graph_snapshot(
    root: &Path,
    source_path: &Path,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
    graph: &ResolutionGraph,
) -> Result<()> {
    let snapshot = GraphSnapshot {
        version: GRAPH_SNAPSHOT_VERSION,
        source: SnapshotSource::capture(source_path)?,
        root_specs_hash: root_specs_hash(required_root, optional_root),
        graph: graph.clone(),
    };

    let path = root.join(GRAPH_SNAPSHOT_FILE);
    let data = bincode::serialize(&snapshot).map_err(|source| SnpmError::SerializeJson {
        path: path.clone(),
        reason: source.to_string(),
    })?;

    fs::write(&path, data).map_err(|source| SnpmError::WriteFile { path, source })
}

pub(crate) fn load_graph_snapshot(root: &Path, source_path: &Path) -> Option<LoadedGraphSnapshot> {
    let path = root.join(GRAPH_SNAPSHOT_FILE);
    let bytes = fs::read(&path).ok()?;
    let snapshot = bincode::deserialize::<GraphSnapshot>(&bytes).ok()?;

    if snapshot.version != GRAPH_SNAPSHOT_VERSION {
        return None;
    }

    let current_source = SnapshotSource::capture(source_path).ok()?;
    if snapshot.source != current_source {
        return None;
    }

    Some(LoadedGraphSnapshot {
        graph: snapshot.graph,
        root_specs_hash: snapshot.root_specs_hash,
    })
}

pub(crate) fn root_specs_hash(
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
) -> String {
    let mut hasher = Sha256::new();

    for (name, value) in required_root {
        hasher.update(b"required\0");
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }

    for (name, value) in optional_root {
        hasher.update(b"optional\0");
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(value.as_bytes());
        hasher.update([0]);
    }

    format!("{:x}", hasher.finalize())
}

impl LoadedGraphSnapshot {
    pub(crate) fn matches_root_specs(
        &self,
        required_root: &BTreeMap<String, String>,
        optional_root: &BTreeMap<String, String>,
    ) -> bool {
        self.root_specs_hash == root_specs_hash(required_root, optional_root)
    }
}

impl SnapshotSource {
    fn capture(path: &Path) -> Result<Self> {
        let metadata = fs::metadata(path).map_err(|source| SnpmError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        let modified = metadata.modified().map_err(|source| SnpmError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        let duration = modified
            .duration_since(UNIX_EPOCH)
            .map_err(|source| SnpmError::Io {
                path: path.to_path_buf(),
                source: std::io::Error::other(source),
            })?;

        Ok(Self {
            path: path.to_path_buf(),
            length: metadata.len(),
            modified_seconds: duration.as_secs(),
            modified_nanoseconds: duration.subsec_nanos(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{load_graph_snapshot, write_graph_snapshot};
    use crate::resolve::{
        PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency,
    };

    use std::collections::BTreeMap;
    use std::fs;
    use std::thread;
    use std::time::Duration;
    use tempfile::tempdir;

    fn graph_fixture() -> ResolutionGraph {
        let id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };

        ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::from([(
                    "dep".to_string(),
                    RootDependency {
                        requested: "^1.0.0".to_string(),
                        resolved: id.clone(),
                    },
                )]),
            },
            packages: BTreeMap::from([(
                id.clone(),
                ResolvedPackage {
                    id,
                    tarball: "https://example.com/dep.tgz".to_string(),
                    integrity: None,
                    dependencies: BTreeMap::new(),
                    peer_dependencies: BTreeMap::new(),
                    bundled_dependencies: None,
                    has_bin: false,
                    bin: None,
                },
            )]),
        }
    }

    #[test]
    fn graph_snapshot_round_trips_when_source_matches() {
        let dir = tempdir().unwrap();
        let source_path = dir.path().join("snpm-lock.yaml");
        fs::write(
            &source_path,
            "version: 1\nroot:\n  dependencies: {}\npackages: {}\n",
        )
        .unwrap();

        let required = BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]);
        let graph = graph_fixture();

        write_graph_snapshot(
            dir.path(),
            &source_path,
            &required,
            &BTreeMap::new(),
            &graph,
        )
        .unwrap();

        let loaded = load_graph_snapshot(dir.path(), &source_path).unwrap();
        assert!(loaded.matches_root_specs(&required, &BTreeMap::new()));
        assert_eq!(loaded.graph.packages.len(), 1);
    }

    #[test]
    fn graph_snapshot_invalidates_when_source_changes() {
        let dir = tempdir().unwrap();
        let source_path = dir.path().join("snpm-lock.yaml");
        fs::write(
            &source_path,
            "version: 1\nroot:\n  dependencies: {}\npackages: {}\n",
        )
        .unwrap();

        let graph = graph_fixture();
        write_graph_snapshot(
            dir.path(),
            &source_path,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &graph,
        )
        .unwrap();

        thread::sleep(Duration::from_millis(5));
        fs::write(
            &source_path,
            "version: 1\nroot:\n  dependencies:\n    dep:\n      requested: ^1.0.0\npackages: {}\n",
        )
        .unwrap();

        assert!(load_graph_snapshot(dir.path(), &source_path).is_none());
    }
}
