use crate::resolve::{PackageId, ResolutionGraph, ResolvedPackage};
use crate::store;
use crate::{Project, Workspace};
use crate::{Result, SnpmConfig, SnpmError, lockfile};
use futures::stream::{self, StreamExt};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::console;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallScenario {
    Hot,
    WarmLinkOnly,
    WarmPartialCache,
    Cold,
}

pub struct CacheCheckResult {
    pub cached: BTreeMap<PackageId, PathBuf>,
    pub missing: Vec<ResolvedPackage>,
}

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub requested: Vec<String>,
    pub dev: bool,
    pub include_dev: bool,
    pub frozen_lockfile: bool,
    pub force: bool,
    pub silent_summary: bool,
}

#[derive(Debug, Clone)]
pub struct InstallResult {
    pub package_count: usize,
    pub elapsed_seconds: f32,
}

#[derive(Debug)]
pub struct OutdatedEntry {
    pub name: String,
    pub current: Option<String>,
    pub wanted: String,
}

#[derive(Debug, Clone)]
pub struct ParsedSpec {
    pub name: String,
    pub range: String,
    pub protocol: Option<String>,
}

pub struct ScenarioResult {
    pub scenario: InstallScenario,
    pub lockfile: Option<lockfile::Lockfile>,
    pub cache_check: Option<CacheCheckResult>,
    pub graph: Option<crate::resolve::ResolutionGraph>,
    pub integrity_state: Option<IntegrityState>,
}

#[derive(Debug, Clone)]
pub struct IntegrityState {
    pub lockfile_hash: String,
    pub patch_hash: String,
}

pub fn detect_install_scenario(
    project: &Project,
    lockfile_path: &Path,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
    config: &SnpmConfig,
    force: bool,
) -> ScenarioResult {
    if !lockfile_path.is_file() {
        console::verbose("scenario: Cold (no lockfile)");
        return ScenarioResult {
            scenario: InstallScenario::Cold,
            lockfile: None,
            cache_check: None,
            graph: None,
            integrity_state: None,
        };
    }

    let existing = match lockfile::read(lockfile_path) {
        Ok(lockfile) => lockfile,
        Err(_) => {
            console::verbose("scenario: Cold (lockfile unreadable)");
            return ScenarioResult {
                scenario: InstallScenario::Cold,
                lockfile: None,
                cache_check: None,
                graph: None,
                integrity_state: None,
            };
        }
    };

    if !lockfile::root_specs_match(&existing, required_root, optional_root) {
        console::verbose("scenario: Cold (lockfile doesn't match manifest)");
        return ScenarioResult {
            scenario: InstallScenario::Cold,
            lockfile: Some(existing),
            cache_check: None,
            graph: None,
            integrity_state: None,
        };
    }

    let graph = lockfile::to_graph(&existing);
    let integrity_state = match build_project_integrity_state(project, &graph) {
        Ok(state) => state,
        Err(error) => {
            console::warn(&format!(
                "scenario: Cold (failed to compute install integrity state: {})",
                error
            ));
            return ScenarioResult {
                scenario: InstallScenario::Cold,
                lockfile: Some(existing),
                cache_check: None,
                graph: None,
                integrity_state: None,
            };
        }
    };

    if !force && check_integrity_file(project, &integrity_state) {
        console::verbose("scenario: Hot (lockfile + node_modules valid)");
        return ScenarioResult {
            scenario: InstallScenario::Hot,
            lockfile: Some(existing),
            cache_check: None,
            graph: Some(graph),
            integrity_state: Some(integrity_state),
        };
    }

    let cache_check = check_store_cache(config, &graph);
    let missing_count = cache_check.missing.len();
    let total_count = graph.packages.len();

    if missing_count == 0 {
        console::verbose(&format!(
            "scenario: WarmLinkOnly ({} packages all cached)",
            total_count
        ));
        return ScenarioResult {
            scenario: InstallScenario::WarmLinkOnly,
            lockfile: Some(existing),
            cache_check: Some(cache_check),
            graph: Some(graph),
            integrity_state: Some(integrity_state),
        };
    }

    console::verbose(&format!(
        "scenario: WarmPartialCache ({}/{} packages cached, {} missing)",
        total_count - missing_count,
        total_count,
        missing_count
    ));
    ScenarioResult {
        scenario: InstallScenario::WarmPartialCache,
        lockfile: Some(existing),
        cache_check: Some(cache_check),
        graph: Some(graph),
        integrity_state: Some(integrity_state),
    }
}

pub fn check_store_cache(config: &SnpmConfig, graph: &ResolutionGraph) -> CacheCheckResult {
    use rayon::prelude::*;

    let base = config.packages_dir();
    let packages: Vec<_> = graph.packages.values().collect();

    let results: Vec<_> = packages
        .par_iter()
        .map(|package| {
            let name_dir = package.id.name.replace('/', "_");
            let package_directory = base.join(&name_dir).join(&package.id.version);
            let marker = package_directory.join(".snpm_complete");

            if marker.is_file() {
                let candidate = package_directory.join("package");
                let root = if candidate.is_dir() {
                    candidate
                } else {
                    package_directory
                };
                (Some((package.id.clone(), root)), None)
            } else {
                (None, Some((*package).clone()))
            }
        })
        .collect();

    let mut cached = BTreeMap::new();
    let mut missing = Vec::new();

    for (hit, miss) in results {
        if let Some((id, path)) = hit {
            cached.insert(id, path);
        }
        if let Some(package) = miss {
            missing.push(package);
        }
    }

    CacheCheckResult { cached, missing }
}

pub async fn materialize_missing_packages(
    config: &SnpmConfig,
    missing: &[ResolvedPackage],
    client: &reqwest::Client,
) -> Result<BTreeMap<PackageId, PathBuf>> {
    if missing.is_empty() {
        return Ok(BTreeMap::new());
    }

    let total = missing.len();
    let progress_count = Arc::new(AtomicUsize::new(0));
    let concurrency = config.registry_concurrency;

    let mut paths = BTreeMap::new();

    let mut results = stream::iter(missing.iter().map(|package| {
        let config = config.clone();
        let client = client.clone();
        let package = package.clone();
        let count = progress_count.clone();

        async move {
            let current = count.fetch_add(1, Ordering::Relaxed) + 1;
            console::progress("📦", &package.id.name, current, total);

            let path = store::ensure_package(&config, &package, &client).await?;
            Ok::<(PackageId, PathBuf), SnpmError>((package.id.clone(), path))
        }
    }))
    .buffer_unordered(concurrency);

    while let Some(result) = results.next().await {
        let (id, path) = result?;
        paths.insert(id, path);
    }

    Ok(paths)
}

pub async fn materialize_store(
    config: &SnpmConfig,
    graph: &ResolutionGraph,
    client: &reqwest::Client,
) -> Result<BTreeMap<PackageId, PathBuf>> {
    let concurrency = config.registry_concurrency;
    let mut paths = BTreeMap::new();

    let mut results = stream::iter(graph.packages.values().map(|package| {
        let config = config.clone();
        let client = client.clone();
        let package = package.clone();

        async move {
            let path = store::ensure_package(&config, &package, &client).await?;
            let id = package.id.clone();
            Ok::<(PackageId, PathBuf), crate::SnpmError>((id, path))
        }
    }))
    .buffer_unordered(concurrency);

    while let Some(result) = results.next().await {
        let (id, path) = result?;
        paths.insert(id, path);
    }

    Ok(paths)
}

pub fn compute_lockfile_hash(graph: &ResolutionGraph) -> String {
    let mut hasher = Sha256::new();

    for (name, dep) in graph.root.dependencies.iter() {
        hasher.update(name.as_bytes());
        hasher.update(dep.requested.as_bytes());
        hasher.update(dep.resolved.name.as_bytes());
        hasher.update(dep.resolved.version.as_bytes());
    }

    for (id, package) in graph.packages.iter() {
        hasher.update(id.name.as_bytes());
        hasher.update(id.version.as_bytes());
        hasher.update(package.id.name.as_bytes());
        hasher.update(package.id.version.as_bytes());
    }

    format!("{:x}", hasher.finalize())
}

pub fn build_project_integrity_state(
    project: &Project,
    graph: &ResolutionGraph,
) -> Result<IntegrityState> {
    Ok(IntegrityState {
        lockfile_hash: compute_lockfile_hash(graph),
        patch_hash: compute_project_patch_hash(project)?,
    })
}

pub fn build_workspace_integrity_state(
    workspace: &Workspace,
    graph: &ResolutionGraph,
) -> Result<IntegrityState> {
    Ok(IntegrityState {
        lockfile_hash: compute_lockfile_hash(graph),
        patch_hash: compute_workspace_patch_hash(workspace)?,
    })
}

pub fn compute_project_patch_hash(project: &Project) -> Result<String> {
    let patched_dependencies = crate::patch::get_patched_dependencies(project);
    if patched_dependencies.is_empty() {
        return Ok("none".to_string());
    }

    let mut hasher = Sha256::new();

    for (key, rel_path) in patched_dependencies {
        hasher.update(key.as_bytes());
        hasher.update(rel_path.as_bytes());

        let patch_path = project.root.join(&rel_path);
        if patch_path.is_file() {
            let bytes = fs::read(&patch_path).map_err(|source| SnpmError::ReadFile {
                path: patch_path,
                source,
            })?;
            hasher.update(&bytes);
        } else {
            hasher.update(b"__missing__");
        }
    }

    Ok(format!("{:x}", hasher.finalize()))
}

pub fn compute_workspace_patch_hash(workspace: &Workspace) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut has_any_patches = false;
    let mut projects: Vec<&Project> = workspace.projects.iter().collect();
    projects.sort_by(|a, b| a.root.cmp(&b.root));

    for project in projects {
        let patch_hash = compute_project_patch_hash(project)?;
        if patch_hash != "none" {
            has_any_patches = true;
        }

        hasher.update(project.root.display().to_string().as_bytes());
        hasher.update(patch_hash.as_bytes());
    }

    if has_any_patches {
        Ok(format!("{:x}", hasher.finalize()))
    } else {
        Ok("none".to_string())
    }
}

pub fn check_integrity_file(project: &Project, state: &IntegrityState) -> bool {
    check_integrity_path(&project.root.join("node_modules"), state)
}

pub fn write_integrity_file(project: &Project, state: &IntegrityState) -> Result<()> {
    write_integrity_path(&project.root.join("node_modules"), state)
}

pub fn check_integrity_path(node_modules: &Path, state: &IntegrityState) -> bool {
    let integrity_path = node_modules.join(".snpm-integrity");

    match fs::read_to_string(&integrity_path) {
        Ok(content) => {
            content == integrity_content(state)
                || (state.patch_hash == "none" && content == legacy_integrity_content(state))
        }
        Err(_) => false,
    }
}

pub fn write_integrity_path(node_modules: &Path, state: &IntegrityState) -> Result<()> {
    if !node_modules.is_dir() {
        return Ok(());
    }

    let integrity_path = node_modules.join(".snpm-integrity");
    let content = integrity_content(state);

    fs::write(&integrity_path, content).map_err(|source| SnpmError::WriteFile {
        path: integrity_path,
        source,
    })
}

fn integrity_content(state: &IntegrityState) -> String {
    format!(
        "lockfile: {}\npatches: {}\n",
        state.lockfile_hash, state.patch_hash
    )
}

fn legacy_integrity_content(state: &IntegrityState) -> String {
    format!("lockfile: {}\n", state.lockfile_hash)
}

pub fn can_any_scripts_run(config: &SnpmConfig, workspace: Option<&crate::Workspace>) -> bool {
    if !config.allow_scripts.is_empty() {
        return true;
    }

    if let Some(ws) = workspace {
        if !ws.config.only_built_dependencies.is_empty() {
            return true;
        }

        if !ws.config.ignored_built_dependencies.is_empty() {
            return true;
        }
    }

    false
}
