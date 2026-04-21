use crate::config::HoistingMode;
use crate::linker::fs::package_node_modules;
use crate::linker::hoist::effective_hoisting;
use crate::operations::install::workspace::is_local_workspace_dependency;
use crate::resolve::{PackageId, ResolutionGraph};
use crate::store::PACKAGE_METADATA_FILE;
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub(crate) const LAYOUT_STATE_FILE: &str = ".snpm-install-state";
const LAYOUT_STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct LayoutState {
    version: u32,
    layout_hash: String,
    checks: Vec<LayoutCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum LayoutCheck {
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

#[cfg(test)]
pub(crate) fn capture_project_layout_state(
    config: &SnpmConfig,
    project: &Project,
    workspace: Option<&Workspace>,
    graph: &ResolutionGraph,
    include_dev: bool,
) -> Result<()> {
    let state = LayoutState {
        version: LAYOUT_STATE_VERSION,
        layout_hash: build_project_layout_hash(config, project, workspace, graph, include_dev)?,
        checks: capture_project_checks(config, project, workspace, graph, include_dev)?,
    };

    write_layout_state(&project.root.join("node_modules"), &state)
}

pub(crate) fn check_project_layout_state(
    config: &SnpmConfig,
    project: &Project,
    workspace: Option<&Workspace>,
    graph: &ResolutionGraph,
    include_dev: bool,
) -> bool {
    if let Some(valid) = super::install_state::check_project_layout_from_install_state(
        config,
        project,
        workspace,
        graph,
        include_dev,
    ) {
        return valid;
    }

    let layout_hash =
        match build_project_layout_hash(config, project, workspace, graph, include_dev) {
            Ok(hash) => hash,
            Err(_) => return false,
        };

    check_layout_state(&project.root.join("node_modules"), &layout_hash)
}

pub(crate) fn check_workspace_layout_state(
    config: &SnpmConfig,
    workspace: &Workspace,
    graph: &ResolutionGraph,
    include_dev: bool,
) -> bool {
    if let Some(valid) = super::install_state::check_workspace_layout_from_install_state(
        config,
        workspace,
        graph,
        include_dev,
    ) {
        return valid;
    }

    let layout_hash = match build_workspace_layout_hash(config, workspace, graph, include_dev) {
        Ok(hash) => hash,
        Err(_) => return false,
    };

    check_layout_state(&workspace.root.join("node_modules"), &layout_hash)
}

pub(crate) fn build_project_layout_hash(
    config: &SnpmConfig,
    project: &Project,
    workspace: Option<&Workspace>,
    graph: &ResolutionGraph,
    include_dev: bool,
) -> Result<String> {
    let mut entries = Vec::new();
    entries.push("kind=project".to_string());
    entries.push(format!(
        "hoisting={}",
        hoisting_label(effective_hoisting(config, workspace))
    ));

    append_virtual_store_entries(&mut entries, config, workspace, graph);
    append_project_link_entries(&mut entries, project, workspace, graph, include_dev)?;
    append_hoist_entries(&mut entries, graph, effective_hoisting(config, workspace));

    Ok(hash_entries(&entries))
}

pub(crate) fn build_workspace_layout_hash(
    config: &SnpmConfig,
    workspace: &Workspace,
    graph: &ResolutionGraph,
    include_dev: bool,
) -> Result<String> {
    let mut entries = Vec::new();
    entries.push("kind=workspace".to_string());

    append_virtual_store_entries(&mut entries, config, Some(workspace), graph);

    for project in &workspace.projects {
        entries.push(format!("project={}", project.root.display()));
        append_project_link_entries(&mut entries, project, Some(workspace), graph, include_dev)?;
    }

    Ok(hash_entries(&entries))
}

fn append_virtual_store_entries(
    entries: &mut Vec<String>,
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    graph: &ResolutionGraph,
) {
    for id in graph.packages.keys() {
        let locality = if crate::lifecycle::is_dep_script_allowed(config, workspace, &id.name) {
            "local"
        } else {
            "shared"
        };
        entries.push(format!("package:{locality}:{}@{}", id.name, id.version));
    }
}

fn append_project_link_entries(
    entries: &mut Vec<String>,
    project: &Project,
    workspace: Option<&Workspace>,
    graph: &ResolutionGraph,
    include_dev: bool,
) -> Result<()> {
    append_manifest_link_entries(
        entries,
        &project.manifest.dependencies,
        "dependencies",
        workspace,
        graph,
    )?;

    if include_dev {
        append_manifest_link_entries(
            entries,
            &project.manifest.dev_dependencies,
            "dev_dependencies",
            workspace,
            graph,
        )?;
    }

    append_manifest_link_entries(
        entries,
        &project.manifest.optional_dependencies,
        "optional_dependencies",
        workspace,
        graph,
    )?;

    Ok(())
}

fn append_manifest_link_entries(
    entries: &mut Vec<String>,
    deps: &BTreeMap<String, String>,
    section: &str,
    workspace: Option<&Workspace>,
    graph: &ResolutionGraph,
) -> Result<()> {
    for (name, spec) in deps {
        if let Some(workspace) = workspace
            && is_local_workspace_dependency(workspace, name, spec)?
        {
            let source_project =
                workspace
                    .project_by_name(name)
                    .ok_or_else(|| SnpmError::WorkspaceConfig {
                        path: workspace.root.clone(),
                        reason: format!(
                            "workspace dependency {name} not found in workspace projects"
                        ),
                    })?;

            entries.push(format!(
                "{section}:workspace:{name}={}",
                source_project.root.display()
            ));
            continue;
        }

        if let Some(root_dep) = graph.root.dependencies.get(name) {
            entries.push(format!(
                "{section}:external:{name}={}@{}",
                root_dep.resolved.name, root_dep.resolved.version
            ));
        }
    }

    Ok(())
}

fn append_hoist_entries(entries: &mut Vec<String>, graph: &ResolutionGraph, mode: HoistingMode) {
    if matches!(mode, HoistingMode::None) {
        return;
    }

    let mut ids_by_name: BTreeMap<&str, Vec<&PackageId>> = BTreeMap::new();
    for id in graph.packages.keys() {
        ids_by_name.entry(&id.name).or_default().push(id);
    }

    for (name, ids) in ids_by_name {
        let should_hoist = match mode {
            HoistingMode::None => false,
            HoistingMode::SingleVersion => ids.len() == 1,
            HoistingMode::All => !ids.is_empty(),
        };

        if should_hoist {
            entries.push(format!("hoist:{name}={}@{}", ids[0].name, ids[0].version));
        }
    }
}

pub(crate) fn capture_project_checks(
    config: &SnpmConfig,
    project: &Project,
    workspace: Option<&Workspace>,
    graph: &ResolutionGraph,
    include_dev: bool,
) -> Result<Vec<LayoutCheck>> {
    let mut checks = Vec::new();
    let mut seen_links = BTreeSet::new();
    let node_modules = project.root.join("node_modules");

    capture_node_modules_checks(&node_modules, &mut checks)?;
    capture_project_link_checks(
        config,
        project,
        workspace,
        graph,
        include_dev,
        true,
        &node_modules,
        &mut checks,
        &mut seen_links,
    )?;
    capture_virtual_store_checks(&project.root.join(".snpm"), graph, &mut checks)?;

    Ok(checks)
}

pub(crate) fn capture_workspace_checks(
    config: &SnpmConfig,
    workspace: &Workspace,
    graph: &ResolutionGraph,
    include_dev: bool,
) -> Result<Vec<LayoutCheck>> {
    let mut checks = Vec::new();

    capture_directory_mtime(&workspace.root.join("node_modules"), &mut checks)?;
    capture_virtual_store_checks(&workspace.root.join(".snpm"), graph, &mut checks)?;

    for project in &workspace.projects {
        let node_modules = project.root.join("node_modules");
        let mut seen_links = BTreeSet::new();

        capture_node_modules_checks(&node_modules, &mut checks)?;
        capture_project_link_checks(
            config,
            project,
            Some(workspace),
            graph,
            include_dev,
            false,
            &node_modules,
            &mut checks,
            &mut seen_links,
        )?;
    }

    Ok(checks)
}

fn capture_project_link_checks(
    config: &SnpmConfig,
    project: &Project,
    workspace: Option<&Workspace>,
    graph: &ResolutionGraph,
    include_dev: bool,
    include_hoists: bool,
    node_modules: &Path,
    checks: &mut Vec<LayoutCheck>,
    seen_links: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    capture_manifest_link_checks(
        &project.manifest.dependencies,
        workspace,
        graph,
        node_modules,
        checks,
        seen_links,
    )?;

    if include_dev {
        capture_manifest_link_checks(
            &project.manifest.dev_dependencies,
            workspace,
            graph,
            node_modules,
            checks,
            seen_links,
        )?;
    }

    capture_manifest_link_checks(
        &project.manifest.optional_dependencies,
        workspace,
        graph,
        node_modules,
        checks,
        seen_links,
    )?;

    let mode = effective_hoisting(config, workspace);

    if include_hoists && !matches!(mode, HoistingMode::None) {
        let mut ids_by_name: BTreeMap<&str, Vec<&PackageId>> = BTreeMap::new();
        for id in graph.packages.keys() {
            ids_by_name.entry(&id.name).or_default().push(id);
        }

        for (name, ids) in ids_by_name {
            let should_hoist = match mode {
                HoistingMode::None => false,
                HoistingMode::SingleVersion => ids.len() == 1,
                HoistingMode::All => !ids.is_empty(),
            };

            if should_hoist {
                capture_link_or_exists(&node_modules.join(name), checks, seen_links)?;
            }
        }
    }

    Ok(())
}

fn capture_manifest_link_checks(
    deps: &BTreeMap<String, String>,
    workspace: Option<&Workspace>,
    graph: &ResolutionGraph,
    node_modules: &Path,
    checks: &mut Vec<LayoutCheck>,
    seen_links: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    for (name, spec) in deps {
        let is_workspace_link = if let Some(workspace) = workspace {
            is_local_workspace_dependency(workspace, name, spec)?
        } else {
            false
        };

        if !is_workspace_link && !graph.root.dependencies.contains_key(name) {
            continue;
        }

        capture_link_or_exists(&node_modules.join(name), checks, seen_links)?;
    }

    Ok(())
}

fn capture_node_modules_checks(node_modules: &Path, checks: &mut Vec<LayoutCheck>) -> Result<()> {
    capture_directory_mtime(node_modules, checks)?;
    capture_directory_mtime_if_exists(&node_modules.join(".bin"), checks)?;

    let entries = match fs::read_dir(node_modules) {
        Ok(entries) => entries,
        Err(source) => {
            return Err(SnpmError::ReadFile {
                path: node_modules.to_path_buf(),
                source,
            });
        }
    };

    for entry in entries {
        let entry = entry.map_err(|source| SnpmError::ReadFile {
            path: node_modules.to_path_buf(),
            source,
        })?;

        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with('@') {
            continue;
        }

        capture_directory_mtime(&entry.path(), checks)?;
    }

    Ok(())
}

fn capture_virtual_store_checks(
    virtual_store_dir: &Path,
    graph: &ResolutionGraph,
    checks: &mut Vec<LayoutCheck>,
) -> Result<()> {
    capture_directory_mtime(virtual_store_dir, checks)?;

    for (id, package) in &graph.packages {
        let package_location = virtual_package_location(virtual_store_dir, id);
        let mut seen = BTreeSet::new();
        capture_link_or_exists(&package_location, checks, &mut seen)?;

        if !package.dependencies.is_empty() {
            let dependency_dir = virtual_package_node_modules(&package_location, &id.name)?;
            capture_directory_mtime(&dependency_dir, checks)?;
            capture_directory_mtime_if_exists(&dependency_dir.join(".bin"), checks)?;
        }
    }

    Ok(())
}

fn capture_link_or_exists(
    path: &Path,
    checks: &mut Vec<LayoutCheck>,
    seen_links: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    let canonical_path = path.to_path_buf();
    if !seen_links.insert(canonical_path.clone()) {
        return Ok(());
    }

    let metadata = fs::symlink_metadata(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    if metadata.file_type().is_symlink() {
        let target = fs::read_link(path).map_err(|source| SnpmError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;

        checks.push(LayoutCheck::SymlinkTarget {
            link: canonical_path,
            target,
        });
        return Ok(());
    }

    checks.push(LayoutCheck::Exists {
        path: canonical_path,
    });
    Ok(())
}

fn capture_directory_mtime(path: &Path, checks: &mut Vec<LayoutCheck>) -> Result<()> {
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

    checks.push(LayoutCheck::Mtime {
        path: path.to_path_buf(),
        seconds: duration.as_secs(),
        nanoseconds: duration.subsec_nanos(),
    });
    Ok(())
}

fn capture_directory_mtime_if_exists(path: &Path, checks: &mut Vec<LayoutCheck>) -> Result<()> {
    if path.is_dir() {
        capture_directory_mtime(path, checks)?;
    }

    Ok(())
}

#[cfg(test)]
fn write_layout_state(node_modules: &Path, state: &LayoutState) -> Result<()> {
    if !node_modules.is_dir() {
        return Ok(());
    }

    let content = serde_json::to_vec_pretty(state).map_err(|source| SnpmError::ParseJson {
        path: state_path(node_modules),
        source,
    })?;
    let path = state_path(node_modules);
    fs::write(&path, content).map_err(|source| SnpmError::WriteFile { path, source })
}

fn check_layout_state(node_modules: &Path, expected_layout_hash: &str) -> bool {
    let state = match read_layout_state(node_modules) {
        Some(state) => state,
        None => return false,
    };

    if state.version != LAYOUT_STATE_VERSION || state.layout_hash != expected_layout_hash {
        return false;
    }

    state.checks.iter().all(check_layout)
}

fn read_layout_state(node_modules: &Path) -> Option<LayoutState> {
    let path = state_path(node_modules);
    let content = fs::read(&path).ok()?;
    serde_json::from_slice(&content).ok()
}

fn check_layout(check: &LayoutCheck) -> bool {
    match check {
        LayoutCheck::Exists { path } => package_path_ready(path),
        LayoutCheck::Mtime {
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
        LayoutCheck::SymlinkTarget { link, target } => match fs::read_link(link) {
            Ok(current) => {
                current == *target && package_path_ready(&resolve_symlink_target(link, target))
            }
            Err(_) => false,
        },
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

fn state_path(node_modules: &Path) -> PathBuf {
    install_state_path(node_modules.parent().unwrap_or(node_modules))
}

pub(super) fn package_path_ready(path: &Path) -> bool {
    path.is_dir()
        && (path.join(PACKAGE_METADATA_FILE).is_file()
            || fs::read_dir(path)
                .ok()
                .and_then(|mut entries| entries.next())
                .is_some())
}

pub(crate) fn install_state_path(root: &Path) -> PathBuf {
    root.join(LAYOUT_STATE_FILE)
}

fn hash_entries(entries: &[String]) -> String {
    let mut hasher = Sha256::new();
    for entry in entries {
        hasher.update(entry.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

fn hoisting_label(mode: HoistingMode) -> &'static str {
    match mode {
        HoistingMode::None => "none",
        HoistingMode::SingleVersion => "single-version",
        HoistingMode::All => "all",
    }
}

fn virtual_package_location(virtual_store_dir: &Path, id: &PackageId) -> PathBuf {
    let safe_name = id.name.replace('/', "+");
    virtual_store_dir
        .join(format!("{}@{}", safe_name, id.version))
        .join("node_modules")
        .join(&id.name)
}

fn virtual_package_node_modules(package_location: &Path, package_name: &str) -> Result<PathBuf> {
    package_node_modules(package_location, package_name).ok_or_else(|| SnpmError::Internal {
        reason: format!(
            "virtual package path missing node_modules parent: {}",
            package_location.display()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        capture_project_layout_state, check_project_layout_state, read_layout_state, state_path,
        virtual_package_location, virtual_package_node_modules,
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
        let pkg = ResolvedPackage {
            id: id.clone(),
            tarball: String::new(),
            integrity: None,
            dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: None,
            has_bin: false,
            bin: None,
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
            packages: BTreeMap::from([(id.clone(), pkg)]),
        }
    }

    fn make_graph_with_transitive_dep(
        root_id: &PackageId,
        child_id: &PackageId,
    ) -> ResolutionGraph {
        let root_pkg = ResolvedPackage {
            id: root_id.clone(),
            tarball: String::new(),
            integrity: None,
            dependencies: BTreeMap::from([("child".to_string(), child_id.clone())]),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: None,
            has_bin: false,
            bin: None,
        };
        let child_pkg = ResolvedPackage {
            id: child_id.clone(),
            tarball: String::new(),
            integrity: None,
            dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: None,
            has_bin: false,
            bin: None,
        };

        ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::from([(
                    "dep".to_string(),
                    RootDependency {
                        requested: "^1.0.0".to_string(),
                        resolved: root_id.clone(),
                    },
                )]),
            },
            packages: BTreeMap::from([(root_id.clone(), root_pkg), (child_id.clone(), child_pkg)]),
        }
    }

    #[cfg(unix)]
    #[test]
    fn project_layout_state_accepts_fresh_capture() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));
        let project = make_project(dir.path().join("project"));
        let id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };
        let graph = make_graph(&id);

        fs::create_dir_all(project.root.join("node_modules")).unwrap();
        let package_root = virtual_package_location(&project.root.join(".snpm"), &id);
        fs::create_dir_all(&package_root).unwrap();
        fs::write(package_root.join("package.json"), "{}").unwrap();
        std::os::unix::fs::symlink(&package_root, project.root.join("node_modules/dep")).unwrap();

        capture_project_layout_state(&config, &project, None, &graph, true).unwrap();

        assert!(state_path(&project.root.join("node_modules")).is_file());
        assert!(check_project_layout_state(
            &config, &project, None, &graph, true
        ));
    }

    #[cfg(unix)]
    #[test]
    fn project_layout_state_rejects_broken_shared_symlink() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));
        let project = make_project(dir.path().join("project"));
        let id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };
        let graph = make_graph(&id);

        fs::create_dir_all(project.root.join("node_modules")).unwrap();
        fs::create_dir_all(project.root.join(".snpm/dep@1.0.0/node_modules")).unwrap();
        let shared_target = config
            .virtual_store_dir()
            .join("dep@1.0.0/node_modules/dep");
        fs::create_dir_all(&shared_target).unwrap();
        fs::write(shared_target.join("package.json"), "{}").unwrap();
        std::os::unix::fs::symlink(
            &shared_target,
            virtual_package_location(&project.root.join(".snpm"), &id),
        )
        .unwrap();
        std::os::unix::fs::symlink(
            virtual_package_location(&project.root.join(".snpm"), &id),
            project.root.join("node_modules/dep"),
        )
        .unwrap();

        capture_project_layout_state(&config, &project, None, &graph, true).unwrap();
        assert!(read_layout_state(&project.root.join("node_modules")).is_some());

        fs::remove_dir_all(&shared_target).unwrap();

        assert!(!check_project_layout_state(
            &config, &project, None, &graph, true
        ));
    }

    #[cfg(unix)]
    #[test]
    fn project_layout_state_rejects_removed_root_link() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));
        let project = make_project(dir.path().join("project"));
        let id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };
        let graph = make_graph(&id);

        fs::create_dir_all(project.root.join("node_modules")).unwrap();
        let package_root = virtual_package_location(&project.root.join(".snpm"), &id);
        fs::create_dir_all(&package_root).unwrap();
        fs::write(package_root.join("package.json"), "{}").unwrap();
        std::os::unix::fs::symlink(&package_root, project.root.join("node_modules/dep")).unwrap();

        capture_project_layout_state(&config, &project, None, &graph, true).unwrap();
        assert!(state_path(&project.root.join("node_modules")).is_file());

        fs::remove_file(project.root.join("node_modules/dep")).unwrap();

        assert!(!check_project_layout_state(
            &config, &project, None, &graph, true
        ));
    }

    #[test]
    fn project_layout_state_rejects_empty_copied_package_dir() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));
        let project = make_project(dir.path().join("project"));
        let id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };
        let graph = make_graph(&id);

        let copied_package = project.root.join("node_modules/dep");
        let package_root = virtual_package_location(&project.root.join(".snpm"), &id);

        fs::create_dir_all(&copied_package).unwrap();
        fs::create_dir_all(&package_root).unwrap();
        fs::write(copied_package.join("package.json"), "{}").unwrap();
        fs::write(package_root.join("package.json"), "{}").unwrap();

        capture_project_layout_state(&config, &project, None, &graph, true).unwrap();

        fs::remove_file(copied_package.join("package.json")).unwrap();

        assert!(!check_project_layout_state(
            &config, &project, None, &graph, true
        ));
    }

    #[cfg(unix)]
    #[test]
    fn project_layout_state_rejects_package_dir_replaced_with_file() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));
        let project = make_project(dir.path().join("project"));
        let id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };
        let graph = make_graph(&id);

        fs::create_dir_all(project.root.join("node_modules")).unwrap();
        let package_root = virtual_package_location(&project.root.join(".snpm"), &id);
        fs::create_dir_all(&package_root).unwrap();
        fs::write(package_root.join("package.json"), "{}").unwrap();
        std::os::unix::fs::symlink(&package_root, project.root.join("node_modules/dep")).unwrap();

        capture_project_layout_state(&config, &project, None, &graph, true).unwrap();

        fs::remove_dir_all(&package_root).unwrap();
        fs::write(&package_root, "not a package").unwrap();

        assert!(!check_project_layout_state(
            &config, &project, None, &graph, true
        ));
    }

    #[cfg(unix)]
    #[test]
    fn project_layout_state_captures_virtual_dependency_container_for_shared_packages() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));
        let project = make_project(dir.path().join("project"));
        let dep_id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };
        let child_id = PackageId {
            name: "child".to_string(),
            version: "1.0.0".to_string(),
        };
        let graph = make_graph_with_transitive_dep(&dep_id, &child_id);

        fs::create_dir_all(project.root.join("node_modules")).unwrap();
        fs::create_dir_all(project.root.join(".snpm")).unwrap();

        let shared_dep_target = config
            .virtual_store_dir()
            .join("dep@1.0.0/node_modules/dep");
        fs::create_dir_all(&shared_dep_target).unwrap();
        fs::write(
            shared_dep_target.join("package.json"),
            r#"{"name":"dep","version":"1.0.0"}"#,
        )
        .unwrap();

        let project_dep_location = virtual_package_location(&project.root.join(".snpm"), &dep_id);
        let project_dep_node_modules =
            virtual_package_node_modules(&project_dep_location, &dep_id.name).unwrap();
        fs::create_dir_all(&project_dep_node_modules).unwrap();
        std::os::unix::fs::symlink(&shared_dep_target, &project_dep_location).unwrap();
        std::os::unix::fs::symlink(&project_dep_location, project.root.join("node_modules/dep"))
            .unwrap();

        let child_location = virtual_package_location(&project.root.join(".snpm"), &child_id);
        fs::create_dir_all(&child_location).unwrap();
        fs::write(
            child_location.join("package.json"),
            r#"{"name":"child","version":"1.0.0"}"#,
        )
        .unwrap();

        capture_project_layout_state(&config, &project, None, &graph, true).unwrap();
        assert!(state_path(&project.root.join("node_modules")).is_file());
    }
}
