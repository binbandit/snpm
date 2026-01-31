use crate::Project;
use crate::resolve::{PackageId, ResolutionGraph, ResolvedPackage};
use crate::store;
use crate::{Result, SnpmConfig, SnpmError, lockfile};
use futures::future::join_all;
use reqwest::Client;
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
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
}

pub fn detect_install_scenario(
    project: &Project,
    lockfile_path: &Path,
    manifest_root: &BTreeMap<String, String>,
    config: &SnpmConfig,
    force: bool,
) -> ScenarioResult {
    if !lockfile_path.is_file() {
        console::verbose("scenario: Cold (no lockfile)");
        return ScenarioResult {
            scenario: InstallScenario::Cold,
            lockfile: None,
            cache_check: None,
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
            };
        }
    };

    let mut lock_requested = BTreeMap::new();
    for (name, dep) in existing.root.dependencies.iter() {
        lock_requested.insert(name.clone(), dep.requested.clone());
    }

    if lock_requested != *manifest_root {
        console::verbose("scenario: Cold (lockfile doesn't match manifest)");
        return ScenarioResult {
            scenario: InstallScenario::Cold,
            lockfile: Some(existing),
            cache_check: None,
        };
    }

    let graph = lockfile::to_graph(&existing);
    let lockfile_hash = compute_lockfile_hash(&graph);

    if !force && check_integrity_file(project, &lockfile_hash) {
        console::verbose("scenario: Hot (lockfile + node_modules valid)");
        return ScenarioResult {
            scenario: InstallScenario::Hot,
            lockfile: Some(existing),
            cache_check: None,
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
    }
}

pub fn check_store_cache(config: &SnpmConfig, graph: &ResolutionGraph) -> CacheCheckResult {
    let base = config.packages_dir();
    let mut cached = BTreeMap::new();
    let mut missing = Vec::new();

    for package in graph.packages.values() {
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
            cached.insert(package.id.clone(), root);
        } else {
            missing.push(package.clone());
        }
    }

    CacheCheckResult { cached, missing }
}

pub async fn materialize_missing_packages(
    config: &SnpmConfig,
    missing: &[ResolvedPackage],
) -> Result<BTreeMap<PackageId, PathBuf>> {
    if missing.is_empty() {
        return Ok(BTreeMap::new());
    }

    let client = Client::new();
    let total = missing.len();
    let progress_count = Arc::new(AtomicUsize::new(0));
    let mut futures = Vec::with_capacity(total);

    for package in missing {
        let config = config.clone();
        let client = client.clone();
        let package = package.clone();
        let count = progress_count.clone();

        let future = async move {
            let current = count.fetch_add(1, Ordering::Relaxed) + 1;
            console::progress("📦", &package.id.name, current, total);

            let path = store::ensure_package(&config, &package, &client).await?;
            Ok::<(PackageId, PathBuf), SnpmError>((package.id.clone(), path))
        };

        futures.push(future);
    }

    let results = join_all(futures).await;
    let mut paths = BTreeMap::new();

    for result in results {
        let (id, path) = result?;
        paths.insert(id, path);
    }

    Ok(paths)
}

pub async fn materialize_store(
    config: &SnpmConfig,
    graph: &ResolutionGraph,
) -> Result<BTreeMap<PackageId, PathBuf>> {
    let client = Client::new();
    let mut futures = Vec::new();

    for package in graph.packages.values() {
        let config = config.clone();
        let client = client.clone();
        let package = package.clone();

        let future = async move {
            let path = store::ensure_package(&config, &package, &client).await?;
            let id = package.id.clone();
            Ok::<(PackageId, PathBuf), crate::SnpmError>((id, path))
        };

        futures.push(future);
    }

    let results = join_all(futures).await;
    let mut paths = BTreeMap::new();

    for result in results {
        let (id, path) = result?;
        paths.insert(id, path);
    }

    Ok(paths)
}

pub fn compute_lockfile_hash(graph: &ResolutionGraph) -> String {
    let mut hasher = DefaultHasher::new();

    for (name, dep) in graph.root.dependencies.iter() {
        name.hash(&mut hasher);
        dep.requested.hash(&mut hasher);
        dep.resolved.name.hash(&mut hasher);
        dep.resolved.version.hash(&mut hasher);
    }

    for (id, package) in graph.packages.iter() {
        id.name.hash(&mut hasher);
        id.version.hash(&mut hasher);
        package.id.name.hash(&mut hasher);
        package.id.version.hash(&mut hasher);
    }

    format!("{:x}", hasher.finish())
}

pub fn check_integrity_file(project: &Project, lockfile_hash: &str) -> bool {
    let integrity_path = project.root.join("node_modules/.snpm-integrity");

    match fs::read_to_string(&integrity_path) {
        Ok(content) => {
            let expected = format!("lockfile: {}\n", lockfile_hash);
            content == expected
        }
        Err(_) => false,
    }
}

pub fn write_integrity_file(project: &Project, lockfile_hash: &str) -> Result<()> {
    let node_modules = project.root.join("node_modules");

    if !node_modules.is_dir() {
        return Ok(());
    }

    let integrity_path = node_modules.join(".snpm-integrity");
    let content = format!("lockfile: {}\n", lockfile_hash);

    fs::write(&integrity_path, content).map_err(|source| SnpmError::WriteFile {
        path: integrity_path,
        source,
    })
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
