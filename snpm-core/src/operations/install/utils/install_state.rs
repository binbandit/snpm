use super::graph_snapshot::root_specs_hash;
use super::layout_state::{
    LayoutCheck, build_project_layout_hash, build_workspace_layout_hash, capture_project_checks,
    capture_workspace_checks, install_state_path, package_path_ready,
};
use super::snapshot_graph::SnapshotGraph;
use crate::resolve::ResolutionGraph;
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace};

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

// v2: switched on-disk encoding from bincode to JSON (bincode cannot
// round-trip the untagged enums embedded in the graph snapshot).
const INSTALL_STATE_VERSION: u32 = 2;
const LEGACY_GRAPH_SNAPSHOT_FILE: &str = ".snpm-graph-snapshot.bin";
static NEXT_TMP_WRITE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
struct InstallStateFile {
    version: u32,
    graph_snapshot: GraphSnapshotState,
    layout: LayoutSnapshot,
}

#[derive(Debug, Clone)]
struct GraphSnapshotState {
    source: SnapshotSource,
    root_specs_hash: String,
    graph: ResolutionGraph,
}

/// On-disk mirror of [`InstallStateFile`]. Kept separate from the
/// in-memory type so every consumer keeps working with a real
/// `ResolutionGraph`, while the bytes on disk use [`SnapshotGraph`] —
/// bincode cannot deserialize the untagged enums the real graph embeds.
#[derive(Debug, Serialize, Deserialize)]
struct DiskInstallStateFile {
    version: u32,
    source: SnapshotSource,
    root_specs_hash: String,
    graph: SnapshotGraph,
    layout: LayoutSnapshot,
}

impl DiskInstallStateFile {
    fn from_memory(state: &InstallStateFile) -> Self {
        Self {
            version: state.version,
            source: state.graph_snapshot.source.clone(),
            root_specs_hash: state.graph_snapshot.root_specs_hash.clone(),
            graph: SnapshotGraph::from(&state.graph_snapshot.graph),
            layout: state.layout.clone(),
        }
    }

    fn into_memory(self) -> InstallStateFile {
        InstallStateFile {
            version: self.version,
            graph_snapshot: GraphSnapshotState {
                source: self.source,
                root_specs_hash: self.root_specs_hash,
                graph: self.graph.into(),
            },
            layout: self.layout,
        }
    }
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

#[allow(clippy::too_many_arguments)]
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

pub(crate) fn load_project_install_state_fast(
    config: &SnpmConfig,
    project: &Project,
    workspace: Option<&Workspace>,
    source_path: &Path,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
    include_dev: bool,
) -> Option<CachedInstallState> {
    let state = read_valid_install_state(&install_state_path(&project.root), source_path)?;
    let root_specs_matches =
        state.graph_snapshot.root_specs_hash == root_specs_hash(required_root, optional_root);
    let layout_valid = root_specs_matches
        && fast_project_layout_valid(config, project, workspace, &state, include_dev);

    Some(CachedInstallState {
        graph: state.graph_snapshot.graph,
        root_specs_matches,
        layout_valid,
    })
}

pub(crate) fn load_workspace_install_state_fast(
    config: &SnpmConfig,
    workspace: &Workspace,
    source_path: &Path,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
    include_dev: bool,
) -> Option<CachedInstallState> {
    let state = read_valid_install_state(&install_state_path(&workspace.root), source_path)?;
    let root_specs_matches =
        state.graph_snapshot.root_specs_hash == root_specs_hash(required_root, optional_root);
    let layout_valid =
        root_specs_matches && fast_workspace_layout_valid(config, workspace, &state, include_dev);

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
    let state = read_valid_install_state(&install_state_path(root), source_path)?;

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
    let expected =
        build_project_layout_hash(config, project, workspace, graph, include_dev).ok()?;
    Some(
        expected == state.layout.layout_hash
            && state.layout.checks.iter().all(StoredLayoutCheck::validate),
    )
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
    Some(
        expected == state.layout.layout_hash
            && state.layout.checks.iter().all(StoredLayoutCheck::validate),
    )
}

fn read_valid_install_state(path: &Path, source_path: &Path) -> Option<InstallStateFile> {
    let state = read_install_state(path)?;
    if state.version != INSTALL_STATE_VERSION {
        return None;
    }
    if state.graph_snapshot.source != SnapshotSource::capture(source_path).ok()? {
        return None;
    }

    Some(state)
}

fn read_install_state(path: &Path) -> Option<InstallStateFile> {
    let bytes = fs::read(path).ok()?;
    bincode::deserialize::<DiskInstallStateFile>(&bytes)
        .ok()
        .map(DiskInstallStateFile::into_memory)
}

fn write_install_state(path: &Path, state: &InstallStateFile) -> Result<()> {
    let data = bincode::serialize(&DiskInstallStateFile::from_memory(state)).map_err(|source| {
        SnpmError::SerializeJson {
            path: path.to_path_buf(),
            reason: source.to_string(),
        }
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

fn fast_project_layout_valid(
    config: &SnpmConfig,
    project: &Project,
    workspace: Option<&Workspace>,
    state: &InstallStateFile,
    include_dev: bool,
) -> bool {
    let expected = match build_project_layout_hash(
        config,
        project,
        workspace,
        &state.graph_snapshot.graph,
        include_dev,
    ) {
        Ok(hash) => hash,
        Err(_) => return false,
    };
    if expected != state.layout.layout_hash {
        return false;
    }

    let link_paths =
        match project_link_paths(project, workspace, &state.graph_snapshot.graph, include_dev) {
            Ok(paths) => paths,
            Err(_) => return false,
        };

    validate_fast_layout_checks(
        &state.layout.checks,
        &[project.root.join("node_modules")],
        &[project.root.join(".snpm")],
        &link_paths,
    )
}

fn fast_workspace_layout_valid(
    config: &SnpmConfig,
    workspace: &Workspace,
    state: &InstallStateFile,
    include_dev: bool,
) -> bool {
    let expected = match build_workspace_layout_hash(
        config,
        workspace,
        &state.graph_snapshot.graph,
        include_dev,
    ) {
        Ok(hash) => hash,
        Err(_) => return false,
    };
    if expected != state.layout.layout_hash {
        return false;
    }

    let mut node_modules_roots = Vec::with_capacity(workspace.projects.len() + 1);
    node_modules_roots.push(workspace.root.join("node_modules"));

    let mut link_paths = BTreeSet::new();
    for project in &workspace.projects {
        node_modules_roots.push(project.root.join("node_modules"));
        let project_paths = match project_link_paths(
            project,
            Some(workspace),
            &state.graph_snapshot.graph,
            include_dev,
        ) {
            Ok(paths) => paths,
            Err(_) => return false,
        };
        link_paths.extend(project_paths);
    }

    validate_fast_layout_checks(
        &state.layout.checks,
        &node_modules_roots,
        &[workspace.root.join(".snpm")],
        &link_paths,
    )
}

fn validate_fast_layout_checks(
    checks: &[StoredLayoutCheck],
    node_modules_roots: &[PathBuf],
    virtual_store_roots: &[PathBuf],
    link_paths: &BTreeSet<PathBuf>,
) -> bool {
    if node_modules_roots.iter().any(|path| !path.is_dir())
        || virtual_store_roots.iter().any(|path| !path.is_dir())
    {
        return false;
    }

    let mut validated_links = BTreeSet::new();
    for check in checks {
        match check {
            StoredLayoutCheck::Mtime { path, .. }
                if is_fast_boundary_mtime(path, node_modules_roots, virtual_store_roots) =>
            {
                if !check.validate() {
                    return false;
                }
            }
            StoredLayoutCheck::Exists { path } if link_paths.contains(path) => {
                if !check.validate() {
                    return false;
                }
                validated_links.insert(path.clone());
            }
            StoredLayoutCheck::SymlinkTarget { link, .. } if link_paths.contains(link) => {
                if !check.validate() {
                    return false;
                }
                validated_links.insert(link.clone());
            }
            _ => {}
        }
    }

    link_paths.iter().all(|path| validated_links.contains(path))
}

fn is_fast_boundary_mtime(
    path: &Path,
    node_modules_roots: &[PathBuf],
    virtual_store_roots: &[PathBuf],
) -> bool {
    if virtual_store_roots.iter().any(|root| path == root) {
        return true;
    }

    node_modules_roots
        .iter()
        .any(|root| path == root || is_node_modules_boundary_child(path, root))
}

fn is_node_modules_boundary_child(path: &Path, node_modules: &Path) -> bool {
    if path == node_modules.join(".bin") {
        return true;
    }

    path.parent() == Some(node_modules)
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('@'))
}

fn project_link_paths(
    project: &Project,
    workspace: Option<&Workspace>,
    graph: &ResolutionGraph,
    include_dev: bool,
) -> Result<BTreeSet<PathBuf>> {
    let mut links = BTreeSet::new();
    let node_modules = project.root.join("node_modules");
    append_project_link_paths(
        &project.manifest.dependencies,
        workspace,
        graph,
        &node_modules,
        &mut links,
    )?;

    if include_dev {
        append_project_link_paths(
            &project.manifest.dev_dependencies,
            workspace,
            graph,
            &node_modules,
            &mut links,
        )?;
    }

    append_project_link_paths(
        &project.manifest.optional_dependencies,
        workspace,
        graph,
        &node_modules,
        &mut links,
    )?;

    Ok(links)
}

fn append_project_link_paths(
    deps: &BTreeMap<String, String>,
    workspace: Option<&Workspace>,
    graph: &ResolutionGraph,
    node_modules: &Path,
    links: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    for (name, spec) in deps {
        let is_workspace_link = if let Some(workspace) = workspace {
            crate::operations::install::workspace::is_local_workspace_dependency(
                workspace, name, spec,
            )?
        } else {
            false
        };

        if is_workspace_link || graph.root.dependencies.contains_key(name) {
            links.insert(node_modules.join(name));
        }
    }

    Ok(())
}

impl StoredLayoutCheck {
    fn validate(&self) -> bool {
        match self {
            Self::Exists { path } => package_path_ready(path),
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
                Ok(current) => {
                    current == *target && package_path_ready(&resolve_symlink_target(link, target))
                }
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
        check_project_layout_from_install_state, check_workspace_layout_from_install_state,
        install_state_path, load_graph_snapshot_from_install_state, load_project_install_state,
        load_project_install_state_fast, load_workspace_install_state,
        load_workspace_install_state_fast, write_project_install_state,
        write_workspace_install_state,
    };
    use crate::Project;
    use crate::Workspace;
    use crate::config::{HoistingMode, SnpmConfig};
    use crate::project::Manifest;
    use crate::resolve::{
        PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency,
    };
    use crate::workspace::types::WorkspaceConfig;

    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_config(data_dir: PathBuf) -> SnpmConfig {
        SnpmConfig {
            cache_dir: data_dir.join("cache"),
            data_dir,
            hoisting: HoistingMode::None,
            ..SnpmConfig::for_tests()
        }
    }

    fn make_project(root: PathBuf) -> Project {
        Project {
            manifest_path: root.join("package.json"),
            root,
            manifest: Manifest {
                name: Some("app".to_string()),
                version: Some("1.0.0".to_string()),
                dependencies: BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
                ..Manifest::default()
            },
        }
    }

    fn make_workspace(root: PathBuf) -> Workspace {
        Workspace {
            root: root.clone(),
            projects: vec![make_project(root)],
            config: WorkspaceConfig {
                packages: Vec::new(),
                catalog: BTreeMap::new(),
                catalogs: BTreeMap::new(),
                only_built_dependencies: Vec::new(),
                ignored_built_dependencies: Vec::new(),
                disable_global_virtual_store_for_packages: None,
                hoisting: None,
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
                    // A bin (an `#[serde(untagged)]` enum) must survive the
                    // on-disk round-trip: with the old bincode encoding this
                    // failed to deserialize, silently killing the fast path.
                    has_bin: true,
                    bin: Some(crate::project::BinField::Single("cli.js".to_string())),
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
        fs::write(package_root.join("package.json"), "{}").unwrap();
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

        let fast_state = load_project_install_state_fast(
            &config,
            &project,
            None,
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
            &BTreeMap::new(),
            true,
        )
        .unwrap();

        assert!(fast_state.root_specs_matches);
        assert!(fast_state.layout_valid);
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
        fs::write(package_root.join("package.json"), "{}").unwrap();
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

        let fast_state = load_project_install_state_fast(
            &config,
            &project,
            None,
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
            &BTreeMap::new(),
            true,
        )
        .unwrap();

        assert!(fast_state.root_specs_matches);
        assert!(!fast_state.layout_valid);
        assert_eq!(
            check_project_layout_from_install_state(&config, &project, None, &graph, true),
            Some(false)
        );
    }

    #[cfg(unix)]
    #[test]
    fn project_install_state_marks_package_dir_replaced_with_file_as_stale() {
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
        fs::write(package_root.join("package.json"), "{}").unwrap();
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

        fs::remove_dir_all(&package_root).unwrap();
        fs::write(&package_root, "not a package").unwrap();

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

        let fast_state = load_project_install_state_fast(
            &config,
            &project,
            None,
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
            &BTreeMap::new(),
            true,
        )
        .unwrap();

        assert!(fast_state.root_specs_matches);
        assert!(!fast_state.layout_valid);
        assert_eq!(
            check_project_layout_from_install_state(&config, &project, None, &graph, true),
            Some(false)
        );
    }

    #[cfg(unix)]
    #[test]
    fn workspace_install_state_round_trips_with_valid_layout() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));
        let workspace = make_workspace(dir.path().join("workspace"));
        let project = &workspace.projects[0];
        let id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };
        let graph = make_graph(&id);
        let lockfile_path = workspace.root.join("snpm-lock.yaml");

        fs::create_dir_all(workspace.root.join("node_modules")).unwrap();
        fs::write(
            &lockfile_path,
            "version: 1\nroot:\n  dependencies: {}\npackages: {}\n",
        )
        .unwrap();

        let package_root = virtual_package_location(&workspace.root, &id);
        fs::create_dir_all(&package_root).unwrap();
        fs::write(package_root.join("package.json"), "{}").unwrap();
        std::os::unix::fs::symlink(&package_root, project.root.join("node_modules/dep")).unwrap();

        write_workspace_install_state(
            &config,
            &workspace,
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
            &BTreeMap::new(),
            &graph,
            true,
        )
        .unwrap();

        let state = load_workspace_install_state(
            &config,
            &workspace,
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
            &BTreeMap::new(),
            true,
        )
        .unwrap();

        assert!(install_state_path(&workspace.root).is_file());
        assert!(state.root_specs_matches);
        assert!(state.layout_valid);

        let fast_state = load_workspace_install_state_fast(
            &config,
            &workspace,
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
            &BTreeMap::new(),
            true,
        )
        .unwrap();

        assert!(fast_state.root_specs_matches);
        assert!(fast_state.layout_valid);
        assert_eq!(
            state
                .graph
                .root
                .dependencies
                .get("dep")
                .map(|dep| dep.resolved.version.as_str()),
            Some("1.0.0")
        );
    }

    #[cfg(unix)]
    #[test]
    fn workspace_install_state_marks_removed_links_as_stale() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));
        let workspace = make_workspace(dir.path().join("workspace"));
        let project = &workspace.projects[0];
        let id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };
        let graph = make_graph(&id);
        let lockfile_path = workspace.root.join("snpm-lock.yaml");

        fs::create_dir_all(workspace.root.join("node_modules")).unwrap();
        fs::write(
            &lockfile_path,
            "version: 1\nroot:\n  dependencies: {}\npackages: {}\n",
        )
        .unwrap();

        let package_root = virtual_package_location(&workspace.root, &id);
        fs::create_dir_all(&package_root).unwrap();
        fs::write(package_root.join("package.json"), "{}").unwrap();
        std::os::unix::fs::symlink(&package_root, project.root.join("node_modules/dep")).unwrap();

        write_workspace_install_state(
            &config,
            &workspace,
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
            &BTreeMap::new(),
            &graph,
            true,
        )
        .unwrap();

        fs::remove_file(project.root.join("node_modules/dep")).unwrap();

        let state = load_workspace_install_state(
            &config,
            &workspace,
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
            &BTreeMap::new(),
            true,
        )
        .unwrap();

        assert!(state.root_specs_matches);
        assert!(!state.layout_valid);

        let fast_state = load_workspace_install_state_fast(
            &config,
            &workspace,
            &lockfile_path,
            &BTreeMap::from([("dep".to_string(), "^1.0.0".to_string())]),
            &BTreeMap::new(),
            true,
        )
        .unwrap();

        assert!(fast_state.root_specs_matches);
        assert!(!fast_state.layout_valid);
        assert_eq!(
            check_workspace_layout_from_install_state(&config, &workspace, &graph, true),
            Some(false)
        );
    }
}
