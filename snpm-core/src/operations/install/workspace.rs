use crate::console;
use crate::linker::{bins::link_bins, fs::symlink_dir_entry};
use crate::lockfile;
use crate::operations::install::utils::{
    InstallResult, InstallScenario, check_store_cache, compute_lockfile_hash,
    materialize_missing_packages, materialize_store,
};
use crate::registry::RegistryProtocol;
use crate::resolve::{self, PackageId, ResolutionGraph};
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace};
use rayon::prelude::*;
use reqwest::Client;
use snpm_semver::{RangeSet, Version};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tokio::task::JoinHandle;

use super::manifest::apply_specs;

pub async fn install_workspace(
    config: &SnpmConfig,
    workspace: &mut Workspace,
    include_dev: bool,
    frozen_lockfile: bool,
    force: bool,
) -> Result<InstallResult> {
    let started = Instant::now();

    if workspace.projects.is_empty() {
        return Ok(InstallResult {
            package_count: 0,
            elapsed_seconds: 0.0,
        });
    }

    let registry_client = Client::new();
    let root_dependencies = collect_workspace_root_deps(workspace, include_dev)?;

    if root_dependencies.is_empty() {
        console::summary(0, 0.0);
        return Ok(InstallResult {
            package_count: 0,
            elapsed_seconds: 0.0,
        });
    }

    let root_protocols: BTreeMap<String, RegistryProtocol> = root_dependencies
        .keys()
        .map(|name| (name.clone(), RegistryProtocol::npm()))
        .collect();

    let lockfile_path = workspace.root.join("snpm-lock.yaml");

    if frozen_lockfile || config.frozen_lockfile_default {
        if !lockfile_path.is_file() {
            return Err(SnpmError::Lockfile {
                path: lockfile_path,
                reason: "frozen-lockfile requested but snpm-lock.yaml is missing".into(),
            });
        }
    }

    let (scenario, existing_lockfile) = detect_workspace_scenario(
        workspace,
        &lockfile_path,
        &root_dependencies,
        config,
        force,
    );

    let mut store_paths_map: BTreeMap<PackageId, PathBuf> = BTreeMap::new();

    let graph = match scenario {
        InstallScenario::Hot => {
            console::step("Using cached install");
            let existing = existing_lockfile.expect("Hot scenario requires lockfile");
            lockfile::to_graph(&existing)
        }

        InstallScenario::WarmLinkOnly => {
            let existing = existing_lockfile.expect("WarmLinkOnly requires lockfile");
            let graph = lockfile::to_graph(&existing);
            let cache_check = check_store_cache(config, &graph);
            store_paths_map = cache_check.cached;
            console::step_with_count("Using cached packages", store_paths_map.len());
            graph
        }

        InstallScenario::WarmPartialCache => {
            let existing = existing_lockfile.expect("WarmPartialCache requires lockfile");
            let graph = lockfile::to_graph(&existing);
            let cache_check = check_store_cache(config, &graph);
            store_paths_map = cache_check.cached;

            if !cache_check.missing.is_empty() {
                console::step("Downloading missing packages");
                let downloaded = materialize_missing_packages(config, &cache_check.missing).await?;
                store_paths_map.extend(downloaded);
            }

            console::step_with_count("Resolved and extracted", store_paths_map.len());
            graph
        }

        InstallScenario::Cold => {
            console::step("Resolving workspace dependencies");

            let graph = resolve_workspace_deps(
                config,
                &registry_client,
                &root_dependencies,
                &root_protocols,
                force,
                &mut store_paths_map,
            )
            .await?;

            if store_paths_map.is_empty() && !graph.packages.is_empty() {
                store_paths_map = materialize_store(config, &graph).await?;
            }

            if include_dev {
                lockfile::write(&lockfile_path, &graph)?;
            }

            console::step_with_count("Resolved, downloaded and extracted", store_paths_map.len());
            graph
        }
    };

    let workspace_node_modules = workspace.root.join("node_modules");
    let shared_virtual_store = workspace_node_modules.join(".snpm");

    fs::create_dir_all(&shared_virtual_store).map_err(|source| SnpmError::WriteFile {
        path: shared_virtual_store.clone(),
        source,
    })?;

    console::step("Linking workspace");
    let virtual_store_paths =
        populate_virtual_store(&shared_virtual_store, &graph, &store_paths_map, config)?;

    link_store_dependencies(&virtual_store_paths, &graph)?;

    workspace.projects.par_iter().try_for_each(|project| {
        link_project_dependencies(project, workspace, &graph, &virtual_store_paths, include_dev)
    })?;

    let lockfile_hash = compute_lockfile_hash(&graph);
    write_workspace_integrity(&workspace.root, &lockfile_hash)?;

    if include_dev {
        console::step("Saved lockfile");
    }

    console::clear_steps(if include_dev { 4 } else { 3 });

    let elapsed = started.elapsed();
    let seconds = elapsed.as_secs_f32();
    let package_count = graph.packages.len();

    console::summary(package_count, seconds);

    Ok(InstallResult {
        package_count,
        elapsed_seconds: seconds,
    })
}

async fn resolve_workspace_deps(
    config: &SnpmConfig,
    client: &Client,
    root_deps: &BTreeMap<String, String>,
    root_protocols: &BTreeMap<String, RegistryProtocol>,
    force: bool,
    store_paths: &mut BTreeMap<PackageId, PathBuf>,
) -> Result<ResolutionGraph> {
    use futures::lock::Mutex;

    let paths = Arc::new(Mutex::new(BTreeMap::new()));
    let tasks: Arc<Mutex<Vec<JoinHandle<Result<()>>>>> = Arc::new(Mutex::new(Vec::new()));
    let progress_count = Arc::new(AtomicUsize::new(0));
    let progress_total = Arc::new(AtomicUsize::new(root_deps.len()));

    let min_age = config.min_package_age_days;

    let graph = {
        let paths = paths.clone();
        let tasks = tasks.clone();
        let count = progress_count.clone();
        let total = progress_total.clone();
        let config_clone = config.clone();
        let client_clone = client.clone();

        resolve::resolve(
            config,
            client,
            root_deps,
            root_protocols,
            min_age,
            force,
            None,
            move |package| {
                let config = config_clone.clone();
                let client = client_clone.clone();
                let paths = paths.clone();
                let tasks = tasks.clone();
                let count = count.clone();
                let total = total.clone();
                let name = package.id.name.clone();

                async move {
                    let current = count.fetch_add(1, Ordering::Relaxed) + 1;
                    let mut total_val = total.load(Ordering::Relaxed);
                    if current > total_val {
                        total_val = current;
                        total.store(total_val, Ordering::Relaxed);
                    }
                    console::progress("🚚", &name, current, total_val);

                    let package_id = package.id.clone();
                    let handle = tokio::spawn(async move {
                        let path = crate::store::ensure_package(&config, &package, &client).await?;
                        let mut map = paths.lock().await;
                        map.insert(package_id, path);
                        Ok::<(), SnpmError>(())
                    });

                    let mut guard = tasks.lock().await;
                    guard.push(handle);
                    Ok(())
                }
            },
        )
        .await?
    };

    // Wait for all store tasks
    let handles = {
        let mut guard = tasks.lock().await;
        std::mem::take(&mut *guard)
    };

    for handle in handles {
        handle
            .await
            .map_err(|e| SnpmError::StoreTask {
                reason: e.to_string(),
            })?
            .ok();
    }

    *store_paths = paths.lock().await.clone();
    Ok(graph)
}

fn detect_workspace_scenario(
    workspace: &Workspace,
    lockfile_path: &Path,
    manifest_root: &BTreeMap<String, String>,
    config: &SnpmConfig,
    force: bool,
) -> (InstallScenario, Option<lockfile::Lockfile>) {
    if !lockfile_path.is_file() {
        return (InstallScenario::Cold, None);
    }

    let existing = match lockfile::read(lockfile_path) {
        Ok(lockfile) => lockfile,
        Err(_) => return (InstallScenario::Cold, None),
    };

    let mut lock_requested = BTreeMap::new();
    for (name, dep) in existing.root.dependencies.iter() {
        lock_requested.insert(name.clone(), dep.requested.clone());
    }

    if lock_requested != *manifest_root {
        return (InstallScenario::Cold, Some(existing));
    }

    let graph = lockfile::to_graph(&existing);
    let lockfile_hash = compute_lockfile_hash(&graph);

    if !force && check_workspace_integrity(&workspace.root, &lockfile_hash) {
        return (InstallScenario::Hot, Some(existing));
    }

    let cache_check = check_store_cache(config, &graph);
    if cache_check.missing.is_empty() {
        return (InstallScenario::WarmLinkOnly, Some(existing));
    }

    (InstallScenario::WarmPartialCache, Some(existing))
}

fn populate_virtual_store(
    virtual_store_dir: &Path,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    config: &SnpmConfig,
) -> Result<BTreeMap<PackageId, PathBuf>> {
    use std::sync::Mutex;

    let virtual_store_paths = Arc::new(Mutex::new(BTreeMap::new()));
    let packages: Vec<_> = graph.packages.iter().collect();

    packages
        .par_iter()
        .try_for_each(|(id, _)| -> Result<()> {
            let safe_name = id.name.replace('/', "+");
            let virtual_id_dir = virtual_store_dir.join(format!("{}@{}", safe_name, id.version));
            let package_location = virtual_id_dir.join("node_modules").join(&id.name);

            let store_path = store_paths.get(id).ok_or_else(|| SnpmError::StoreMissing {
                name: id.name.clone(),
                version: id.version.clone(),
            })?;

            if package_location.exists() {
                fs::remove_dir_all(&package_location).ok();
            }

            if let Some(parent) = package_location.parent() {
                fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }

            crate::linker::fs::link_dir(config, store_path, &package_location)?;

            virtual_store_paths
                .lock()
                .unwrap()
                .insert((*id).clone(), package_location);

            Ok(())
        })?;

    Ok(Arc::try_unwrap(virtual_store_paths)
        .unwrap()
        .into_inner()
        .unwrap())
}

fn link_store_dependencies(
    virtual_store_paths: &BTreeMap<PackageId, PathBuf>,
    graph: &ResolutionGraph,
) -> Result<()> {
    let packages: Vec<_> = graph.packages.iter().collect();

    packages.par_iter().try_for_each(|(id, package)| {
        let package_location = virtual_store_paths.get(id).unwrap();
        let package_node_modules = package_location.parent().unwrap();

        for (dep_name, dep_id) in &package.dependencies {
            let dep_target = virtual_store_paths.get(dep_id).unwrap();
            let dep_link = package_node_modules.join(dep_name);

            if let Some(parent) = dep_link.parent() {
                fs::create_dir_all(parent).ok();
            }

            if !dep_link.exists() {
                symlink_dir_entry(dep_target, &dep_link).ok();
            }
        }
        Ok::<(), SnpmError>(())
    })
}

fn link_project_dependencies(
    project: &Project,
    workspace: &Workspace,
    graph: &ResolutionGraph,
    virtual_store_paths: &BTreeMap<PackageId, PathBuf>,
    include_dev: bool,
) -> Result<()> {
    let node_modules = project.root.join("node_modules");

    fs::create_dir_all(&node_modules).map_err(|source| SnpmError::WriteFile {
        path: node_modules.clone(),
        source,
    })?;

    let (workspace_deps, workspace_dev_deps) = collect_workspace_protocol_deps(project);

    link_external_deps(
        &project.manifest.dependencies,
        &workspace_deps,
        graph,
        virtual_store_paths,
        &node_modules,
    )?;

    if include_dev {
        link_external_deps(
            &project.manifest.dev_dependencies,
            &workspace_dev_deps,
            graph,
            virtual_store_paths,
            &node_modules,
        )?;
    }

    link_local_workspace_deps(
        project,
        Some(workspace),
        &workspace_deps,
        &workspace_dev_deps,
        include_dev,
    )
}

fn collect_workspace_protocol_deps(project: &Project) -> (BTreeSet<String>, BTreeSet<String>) {
    let deps = project
        .manifest
        .dependencies
        .iter()
        .filter(|(_, v)| v.starts_with("workspace:"))
        .map(|(k, _)| k.clone())
        .collect();

    let dev_deps = project
        .manifest
        .dev_dependencies
        .iter()
        .filter(|(_, v)| v.starts_with("workspace:"))
        .map(|(k, _)| k.clone())
        .collect();

    (deps, dev_deps)
}

fn link_external_deps(
    manifest_deps: &BTreeMap<String, String>,
    workspace_deps: &BTreeSet<String>,
    graph: &ResolutionGraph,
    virtual_store_paths: &BTreeMap<PackageId, PathBuf>,
    node_modules: &Path,
) -> Result<()> {
    for (name, value) in manifest_deps {
        if value.starts_with("workspace:") || workspace_deps.contains(name) {
            continue;
        }

        if let Some(root_dep) = graph.root.dependencies.get(name) {
            let target = virtual_store_paths.get(&root_dep.resolved).ok_or_else(|| {
                SnpmError::GraphMissing {
                    name: root_dep.resolved.name.clone(),
                    version: root_dep.resolved.version.clone(),
                }
            })?;

            let destination = node_modules.join(name);
            create_symlink(target, &destination)?;
            link_bins(&destination, node_modules, name).ok();
        }
    }
    Ok(())
}

fn create_symlink(target: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).ok();
    }

    if destination.exists() || destination.symlink_metadata().is_ok() {
        if destination.is_dir() {
            fs::remove_dir_all(destination).ok();
        } else {
            fs::remove_file(destination).ok();
        }
    }

    symlink_dir_entry(target, destination).map_err(|source| SnpmError::WriteFile {
        path: destination.to_path_buf(),
        source,
    })
}

fn check_workspace_integrity(workspace_root: &Path, lockfile_hash: &str) -> bool {
    let integrity_path = workspace_root.join("node_modules/.snpm-integrity");
    match fs::read_to_string(&integrity_path) {
        Ok(content) => content == format!("lockfile: {}\n", lockfile_hash),
        Err(_) => false,
    }
}

fn write_workspace_integrity(workspace_root: &Path, lockfile_hash: &str) -> Result<()> {
    let node_modules = workspace_root.join("node_modules");
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

pub fn collect_workspace_root_deps(
    workspace: &Workspace,
    include_dev: bool,
) -> Result<BTreeMap<String, String>> {
    let mut combined = BTreeMap::new();

    for member in workspace.projects.iter() {
        let mut local = BTreeSet::new();
        let dependencies = apply_specs(
            &member.manifest.dependencies,
            Some(workspace),
            None,
            &mut local,
            None,
        )?;

        for (name, range) in dependencies.iter() {
            insert_workspace_root_dep(&mut combined, &workspace.root, name, range)?;
        }

        if include_dev {
            let mut local_development = BTreeSet::new();
            let development_dependencies = apply_specs(
                &member.manifest.dev_dependencies,
                Some(workspace),
                None,
                &mut local_development,
                None,
            )?;

            for (name, range) in development_dependencies.iter() {
                insert_workspace_root_dep(&mut combined, &workspace.root, name, range)?;
            }
        }
    }

    Ok(combined)
}

pub fn insert_workspace_root_dep(
    combined: &mut BTreeMap<String, String>,
    root: &Path,
    name: &str,
    range: &str,
) -> Result<()> {
    if let Some(existing) = combined.get(name) {
        if existing != range {
            return Err(SnpmError::WorkspaceConfig {
                path: root.to_path_buf(),
                reason: format!(
                    "dependency {name} has conflicting ranges {existing} and {range} across workspace projects"
                ),
            });
        }
    } else {
        combined.insert(name.to_string(), range.to_string());
    }

    Ok(())
}

pub fn validate_workspace_spec(workspace: &Workspace, name: &str, spec: &str) -> Result<()> {
    let project = workspace
        .projects
        .iter()
        .find(|p| p.manifest.name.as_deref() == Some(name))
        .ok_or_else(|| SnpmError::WorkspaceConfig {
            path: workspace.root.clone(),
            reason: format!("workspace dependency {name} not found in workspace projects"),
        })?;

    let version =
        project
            .manifest
            .version
            .as_deref()
            .ok_or_else(|| SnpmError::WorkspaceConfig {
                path: workspace.root.clone(),
                reason: format!("workspace dependency {name} has no version in its package.json"),
            })?;

    let suffix = &spec["workspace:".len()..];
    let trimmed = suffix.trim();

    if trimmed.is_empty() || trimmed == "*" {
        return Ok(());
    }

    let range_str = match trimmed {
        "^" => format!("^{}", version),
        "~" => format!("~{}", version),
        other => other.to_string(),
    };

    let ranges = RangeSet::parse(&range_str).map_err(|error| SnpmError::Semver {
        value: format!("{}@{}", name, range_str),
        reason: error.to_string(),
    })?;

    let version_parsed = Version::parse(version).map_err(|error| SnpmError::Semver {
        value: format!("{}@{}", name, version),
        reason: error.to_string(),
    })?;

    if ranges.matches(&version_parsed) {
        Ok(())
    } else {
        Err(SnpmError::WorkspaceConfig {
            path: workspace.root.clone(),
            reason: format!(
                "workspace dependency {name} with spec {spec} is not satisfied by local version {version}"
            ),
        })
    }
}

pub fn link_local_workspace_deps(
    project: &crate::Project,
    workspace: Option<&Workspace>,
    local_deps: &BTreeSet<String>,
    local_dev_deps: &BTreeSet<String>,
    include_dev: bool,
) -> Result<()> {
    if local_deps.is_empty() && local_dev_deps.is_empty() {
        return Ok(());
    }

    let workspace_reference = match workspace {
        Some(w) => w,
        None => {
            return Err(SnpmError::WorkspaceConfig {
                path: project.root.clone(),
                reason: "workspace: protocol used but no workspace configuration found".into(),
            });
        }
    };

    let node_modules = project.root.join("node_modules");

    for name in local_deps.iter().chain(local_dev_deps.iter()) {
        let only_dev = local_dev_deps.contains(name) && !local_deps.contains(name);
        if !include_dev && only_dev {
            continue;
        }

        let source_project = workspace_reference
            .projects
            .iter()
            .find(|p| p.manifest.name.as_deref() == Some(name.as_str()))
            .ok_or_else(|| SnpmError::WorkspaceConfig {
                path: workspace_reference.root.clone(),
                reason: format!("workspace dependency {name} not found in workspace projects"),
            })?;

        let dest = node_modules.join(name);

        if let Some(parent) = dest.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }
        }

        if dest.exists() {
            if dest.is_dir() {
                fs::remove_dir_all(&dest)
            } else {
                fs::remove_file(&dest)
            }
            .map_err(|source| SnpmError::WriteFile {
                path: dest.clone(),
                source,
            })?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(&source_project.root, &dest).map_err(|source| SnpmError::WriteFile {
                path: dest.clone(),
                source,
            })?;
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_dir;
            symlink_dir(&source_project.root, &dest).map_err(|source| SnpmError::WriteFile {
                path: dest.clone(),
                source,
            })?;
        }
    }

    Ok(())
}
