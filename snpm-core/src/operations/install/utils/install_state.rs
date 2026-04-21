use super::graph_snapshot::root_specs_hash;
use super::layout_state::{
    LayoutCheck, build_project_layout_hash, build_workspace_layout_hash, capture_project_checks,
    capture_workspace_checks, install_state_path,
};
use crate::resolve::ResolutionGraph;
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace};

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

const INSTALL_STATE_VERSION: u32 = 1;
const LEGACY_GRAPH_SNAPSHOT_FILE: &str = ".snpm-graph-snapshot.bin";
static NEXT_TMP_WRITE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstallStateFile {
    version: u32,
    graph_snapshot: GraphSnapshotState,
    layout: LayoutSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GraphSnapshotState {
    source: SnapshotSource,
    root_specs_hash: String,
    graph: ResolutionGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LayoutSnapshot {
    layout_hash: String,
    checks: Vec<StoredLayoutCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SnapshotSource {
    path: PathBuf,
    length: u64,
    modified_seconds: u64,
    modified_nanoseconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
enum StoredLayoutCheck {
    Exists {
        path: PathBuf,
    },
    Mtime {
        path: PathBuf,
        seconds: u64,
        nanoseconds: u32,
    },
    SymlinkTarget {
        link: PathBuf,
        target: PathBuf,
    },
}

pub(crate) struct CachedInstallState {
    pub(crate) graph: ResolutionGraph,
    pub(crate) root_specs_matches: bool,
    pub(crate) layout_valid: bool,
}

pub(crate) fn write_project_install_state(
    config: &SnpmConfig,
    project: &Project,
    workspace: Option<&Workspace>,
    source_path: &Path,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
    graph: &ResolutionGraph,
    include_dev: bool,
) -> Result<()> {
    let state = InstallStateFile {
        version: INSTALL_STATE_VERSION,
        graph_snapshot: GraphSnapshotState {
            source: SnapshotSource::capture(source_path)?,
            root_specs_hash: root_specs_hash(required_root, optional_root),
            graph: graph.clone(),
        },
        layout: LayoutSnapshot {
            layout_hash: build_project_layout_hash(config, project, workspace, graph, include_dev)?,
            checks: capture_project_checks(config, project, workspace, graph, include_dev)?
                .into_iter()
                .map(StoredLayoutCheck::from)
                .collect(),
        },
    };

    write_install_state(&install_state_path(&project.root), &state)?;
    fs::remove_file(project.root.join(LEGACY_GRAPH_SNAPSHOT_FILE)).ok();
    Ok(())
}

pub(crate) fn write_workspace_install_state(
    config: &SnpmConfig,
    workspace: &Workspace,
    source_path: &Path,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
    graph: &ResolutionGraph,
    include_dev: bool,
) -> Result<()> {
    let state = InstallStateFile {
        version: INSTALL_STATE_VERSION,
        graph_snapshot: GraphSnapshotState {
            source: SnapshotSource::capture(source_path)?,
            root_specs_hash: root_specs_hash(required_root, optional_root),
            graph: graph.clone(),
        },
        layout: LayoutSnapshot {
            layout_hash: build_workspace_layout_hash(config, workspace, graph, include_dev)?,
            checks: capture_workspace_checks(config, workspace, graph, include_dev)?
                .into_iter()
                .map(StoredLayoutCheck::from)
                .collect(),
        },
    };

    write_install_state(&install_state_path(&workspace.root), &state)?;
    fs::remove_file(workspace.root.join(LEGACY_GRAPH_SNAPSHOT_FILE)).ok();
    Ok(())
}

pub(crate) fn load_project_install_state(
    config: &SnpmConfig,
    project: &Project,
    workspace: Option<&Workspace>,
    source_path: &Path,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
    include_dev: bool,
) -> Option<CachedInstallState> {
    let state = read_install_state(&install_state_path(&project.root))?;
    if state.version != INSTALL_STATE_VERSION {
        return None;
    }
    if state.graph_snapshot.source != SnapshotSource::capture(source_path).ok()? {
        return None;
    }

    let root_specs_matches =
        state.graph_snapshot.root_specs_hash == root_specs_hash(required_root, optional_root);
    let layout_valid = root_specs_matches
        && build_project_layout_hash(
            config,
            project,
            workspace,
            &state.graph_snapshot.graph,
            include_dev,
        )
        .ok()
        .is_some_and(|expected| expected == state.layout.layout_hash)
        && state.layout.checks.iter().all(StoredLayoutCheck::validate);

    Some(CachedInstallState {
        graph: state.graph_snapshot.graph,
        root_specs_matches,
        layout_valid,
    })
}

pub(crate) fn load_workspace_install_state(
    config: &SnpmConfig,
    workspace: &Workspace,
    source_path: &Path,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
    include_dev: bool,
) -> Option<CachedInstallState> {
    let state = read_install_state(&install_state_path(&workspace.root))?;
    if state.version != INSTALL_STATE_VERSION {
        return None;
    }
    if state.graph_snapshot.source != SnapshotSource::capture(source_path).ok()? {
        return None;
    }

    let root_specs_matches =
        state.graph_snapshot.root_specs_hash == root_specs_hash(required_root, optional_root);
    let layout_valid = root_specs_matches
        && build_workspace_layout_hash(config, workspace, &state.graph_snapshot.graph, include_dev)
            .ok()
            .is_some_and(|expected| expected == state.layout.layout_hash)
        && state.layout.checks.iter().all(StoredLayoutCheck::validate);

    Some(CachedInstallState {
        graph: state.graph_snapshot.graph,
        root_specs_matches,
        layout_valid,
    })
}

pub(crate) fn load_graph_snapshot_from_install_state(
    root: &Path,
    source_path: &Path,
) -> Option<(ResolutionGraph, String)> {
    let state = read_install_state(&install_state_path(root))?;
    if state.version != INSTALL_STATE_VERSION {
        return None;
    }
    if state.graph_snapshot.source != SnapshotSource::capture(source_path).ok()? {
        return None;
    }

    Some((
        state.graph_snapshot.graph,
        state.graph_snapshot.root_specs_hash,
    ))
}

pub(crate) fn check_project_layout_from_install_state(
    config: &SnpmConfig,
    project: &Project,
    workspace: Option<&Workspace>,
    graph: &ResolutionGraph,
    include_dev: bool,
) -> Option<bool> {
    let state = read_install_state(&install_state_path(&project.root))?;
    if state.version != INSTALL_STATE_VERSION {
        return None;
    }
    let expected = build_project_layout_hash(config, project, workspace, graph, include_dev).ok()?;
    Some(expected == state.layout.layout_hash && state.layout.checks.iter().all(StoredLayoutCheck::validate))
}

pub(crate) fn check_workspace_layout_from_install_state(
    config: &SnpmConfig,
    workspace: &Workspace,
    graph: &ResolutionGraph,
    include_dev: bool,
) -> Option<bool> {
    let state = read_install_state(&install_state_path(&workspace.root))?;
    if state.version != INSTALL_STATE_VERSION {
        return None;
    }
    let expected = build_workspace_layout_hash(config, workspace, graph, include_dev).ok()?;
    Some(expected == state.layout.layout_hash && state.layout.checks.iter().all(StoredLayoutCheck::validate))
}

fn read_install_state(path: &Path) -> Option<InstallStateFile> {
    let bytes = fs::read(path).ok()?;
    bincode::deserialize(&bytes).ok()
}

fn write_install_state(path: &Path, state: &InstallStateFile) -> Result<()> {
    let data = bincode::serialize(state).map_err(|source| SnpmError::SerializeJson {
        path: path.to_path_buf(),
        reason: source.to_string(),
    })?;
    let parent = path.parent().ok_or_else(|| SnpmError::Internal {
        reason: format!("install state path has no parent: {}", path.display()),
    })?;
    fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
        path: parent.to_path_buf(),
        source,
    })?;
    let tmp_path = parent.join(format!(
        ".snpm-install-state.{}.{}.tmp",
        std::process::id(),
        NEXT_TMP_WRITE_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&tmp_path, data).map_err(|source| SnpmError::WriteFile {
        path: tmp_path.clone(),
        source,
    })?;
    match fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(_source) if path.is_file() => {
            fs::remove_file(path).ok();
            match fs::rename(&tmp_path, path) {
                Ok(()) => Ok(()),
                Err(source) => {
                    fs::remove_file(&tmp_path).ok();
                    Err(SnpmError::WriteFile {
                        path: path.to_path_buf(),
                        source,
                    })
                }
            }
        }
        Err(source) => {
            fs::remove_file(&tmp_path).ok();
            Err(SnpmError::WriteFile {
                path: path.to_path_buf(),
                source,
            })
        }
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

impl From<LayoutCheck> for StoredLayoutCheck {
    fn from(value: LayoutCheck) -> Self {
        match value {
            LayoutCheck::Exists { path } => Self::Exists { path },
            LayoutCheck::Mtime {
                path,
                seconds,
                nanoseconds,
            } => Self::Mtime {
                path,
                seconds,
                nanoseconds,
            },
            LayoutCheck::SymlinkTarget { link, target } => Self::SymlinkTarget { link, target },
        }
    }
}

impl StoredLayoutCheck {
    fn validate(&self) -> bool {
        match self {
            Self::Exists { path } => path.exists(),
            Self::Mtime {
                path,
                seconds,
                nanoseconds,
            } => {
                let Ok(metadata) = fs::metadata(path) else {
                    return false;
                };
                let Ok(modified) = metadata.modified() else {
                    return false;
                };
                let Ok(duration) = modified.duration_since(UNIX_EPOCH) else {
                    return false;
                };

                duration.as_secs() == *seconds && duration.subsec_nanos() == *nanoseconds
            }
            Self::SymlinkTarget { link, target } => match fs::read_link(link) {
                Ok(current) => current == *target && resolve_symlink_target(link, target).exists(),
                Err(_) => false,
            },
        }
    }
}

fn resolve_symlink_target(link: &Path, target: &Path) -> PathBuf {
    if target.is_absolute() {
        return target.to_path_buf();
    }

    link.parent()
        .map(|parent| parent.join(target))
        .unwrap_or_else(|| target.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::{
        check_project_layout_from_install_state, install_state_path,
        load_graph_snapshot_from_install_state, load_project_install_state,
        write_project_install_state,
    };
    use crate::Project;
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
    use crate::project::Manifest;
    use crate::resolve::{
        PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency,
    };

    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_config(data_dir: PathBuf) -> SnpmConfig {
        SnpmConfig {
            cache_dir: data_dir.join("cache"),
            data_dir,
            allow_scripts: BTreeSet::new(),
            min_package_age_days: None,
            min_package_cache_age_days: None,
            default_registry: "https://registry.npmjs.org".to_string(),
            scoped_registries: BTreeMap::new(),
            registry_auth: BTreeMap::new(),
            default_registry_auth_token: None,
            default_registry_auth_scheme: AuthScheme::Bearer,
            registry_auth_schemes: BTreeMap::new(),
            hoisting: HoistingMode::None,
            link_backend: LinkBackend::Auto,
            strict_peers: false,
            frozen_lockfile_default: false,
            always_auth: false,
            registry_concurrency: 64,
            verbose: false,
            log_file: None,
        }
    }

    fn make_project(root: PathBuf) -> Project {
        Project {
            manifest_path: root.join("package.json"),
            root,
            manifest: Manifest {
                name: Some("app".to_string()),
                version: Some("1.0.0".to_string()),
                private: false,
                dependencies: BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
                dev_dependencies: BTreeMap::new(),
                optional_dependencies: BTreeMap::new(),
                scripts: BTreeMap::new(),
                resolutions: BTreeMap::new(),
                files: None,
                bin: None,
                main: None,
                pnpm: None,
                snpm: None,
                workspaces: None,
            },
        }
    }

    fn make_graph(id: &PackageId) -> ResolutionGraph {
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
                    id: id.clone(),
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

    fn virtual_package_location(root: &std::path::Path, id: &PackageId) -> PathBuf {
        root.join(".snpm")
            .join(format!("{}@{}", id.name.replace('/', "+"), id.version))
            .join("node_modules")
            .join(&id.name)
    }

    #[cfg(unix)]
    #[test]
    fn project_install_state_round_trips_with_valid_layout() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));
        let project = make_project(dir.path().join("project"));
        let id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };
        let graph = make_graph(&id);
        let lockfile_path = project.root.join("snpm-lock.yaml");

        fs::create_dir_all(project.root.join("node_modules")).unwrap();
        fs::write(
            &lockfile_path,
            "version: 1\nroot:\n  dependencies: {}\npackages: {}\n",
        )
        .unwrap();

        let package_root = virtual_package_location(&project.root, &id);
        fs::create_dir_all(&package_root).unwrap();
        std::os::unix::fs::symlink(&package_root, project.root.join("node_modules/dep")).unwrap();

        write_project_install_state(
            &config,
            &project,
            None,
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
            &BTreeMap::new(),
            &graph,
            true,
        )
        .unwrap();

        let state = load_project_install_state(
            &config,
            &project,
            None,
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
            &BTreeMap::new(),
            true,
        )
        .unwrap();

        assert!(install_state_path(&project.root).is_file());
        assert!(state.root_specs_matches);
        assert!(state.layout_valid);
        assert_eq!(
            state
                .graph
                .root
                .dependencies
                .get("dep")
                .map(|dep| dep.resolved.version.as_str()),
            Some("1.0.0")
        );

        let snapshot =
            load_graph_snapshot_from_install_state(&project.root, &lockfile_path).unwrap();
        assert_eq!(
            snapshot
                .0
                .root
                .dependencies
                .get("dep")
                .map(|dep| dep.resolved.version.as_str()),
            Some("1.0.0")
        );
    }

    #[cfg(unix)]
    #[test]
    fn project_install_state_marks_removed_links_as_stale() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));
        let project = make_project(dir.path().join("project"));
        let id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };
        let graph = make_graph(&id);
        let lockfile_path = project.root.join("snpm-lock.yaml");

        fs::create_dir_all(project.root.join("node_modules")).unwrap();
        fs::write(
            &lockfile_path,
            "version: 1\nroot:\n  dependencies: {}\npackages: {}\n",
        )
        .unwrap();

        let package_root = virtual_package_location(&project.root, &id);
        fs::create_dir_all(&package_root).unwrap();
        std::os::unix::fs::symlink(&package_root, project.root.join("node_modules/dep")).unwrap();

        write_project_install_state(
            &config,
            &project,
            None,
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
            &BTreeMap::new(),
            &graph,
            true,
        )
        .unwrap();

        fs::remove_file(project.root.join("node_modules/dep")).unwrap();

        let state = load_project_install_state(
            &config,
            &project,
            None,
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
            &BTreeMap::new(),
            true,
        )
        .unwrap();

        assert!(state.root_specs_matches);
        assert!(!state.layout_valid);
        assert_eq!(
            check_project_layout_from_install_state(&config, &project, None, &graph, true),
            Some(false)
        );
    }
}
