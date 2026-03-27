use crate::console;
use crate::lifecycle;
use crate::linker::{bins::link_bins, fs::symlink_dir_entry};
use crate::lockfile;
use crate::operations::install::utils::{
    InstallResult, InstallScenario, build_workspace_integrity_state, can_any_scripts_run,
    check_integrity_path, check_store_cache, compute_project_patch_hash,
    materialize_missing_packages, materialize_store, write_integrity_path,
};
use crate::operations::patch::get_patches_to_apply;
use crate::patch;
use crate::registry::RegistryProtocol;
use crate::resolve::{self, PackageId, ResolutionGraph};
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace, http};
use rayon::prelude::*;
use snpm_semver::RangeSet;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tokio::task::JoinHandle;

use super::manifest::{RootSpecSet, apply_specs, build_project_root_specs};

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

    let registry_client = http::create_client()?;
    let lockfile_path = workspace.root.join("snpm-lock.yaml");

    if (frozen_lockfile || config.frozen_lockfile_default) && !lockfile_path.is_file() {
        return Err(SnpmError::Lockfile {
            path: lockfile_path,
            reason: "frozen-lockfile requested but snpm-lock.yaml is missing".into(),
        });
    }

    let (scenario, existing_lockfile) =
        detect_workspace_scenario_early(workspace, &lockfile_path, config, force);

    let root_specs = collect_workspace_root_specs(workspace, include_dev)?;
    let mut root_dependencies = root_specs.required.clone();

    for (name, range) in root_specs.optional.iter() {
        root_dependencies.insert(name.clone(), range.clone());
    }

    if root_dependencies.is_empty() {
        console::summary(0, 0.0);
        return Ok(InstallResult {
            package_count: 0,
            elapsed_seconds: 0.0,
        });
    }

    let root_protocols: BTreeMap<String, RegistryProtocol> = root_dependencies
        .iter()
        .map(|(name, spec)| {
            let protocol = super::manifest::detect_manifest_protocol(spec)
                .unwrap_or_else(RegistryProtocol::npm);
            (name.clone(), protocol)
        })
        .collect();
    let optional_root_names: BTreeSet<String> = root_specs.optional.keys().cloned().collect();

    let (scenario, existing_lockfile) = validate_lockfile_matches_manifest(
        scenario,
        existing_lockfile,
        &root_specs.required,
        &root_specs.optional,
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
                let downloaded =
                    materialize_missing_packages(config, &cache_check.missing, &registry_client)
                        .await?;
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
                &optional_root_names,
                force,
                &mut store_paths_map,
            )
            .await?;

            if store_paths_map.is_empty() && !graph.packages.is_empty() {
                store_paths_map = materialize_store(config, &graph, &registry_client).await?;
            }

            if include_dev {
                lockfile::write(&lockfile_path, &graph, &root_specs.optional)?;
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

    let virtual_store_paths = if matches!(scenario, InstallScenario::Hot) {
        console::step("Validating workspace structure");
        rebuild_virtual_store_paths(&shared_virtual_store, &graph)?
    } else {
        console::step("Linking workspace");
        populate_virtual_store(&shared_virtual_store, &graph, &store_paths_map, config)?
    };

    link_store_dependencies(&virtual_store_paths, &graph)?;

    workspace.projects.par_iter().try_for_each(|project| {
        link_project_dependencies(
            project,
            workspace,
            &graph,
            &virtual_store_paths,
            include_dev,
        )
    })?;

    let patches_applied = apply_workspace_patches(workspace, &store_paths_map)?;
    if patches_applied > 0 {
        console::verbose(&format!("applied {} workspace patches", patches_applied));
    }

    let blocked_scripts = if can_any_scripts_run(config, Some(workspace)) {
        let roots: Vec<&Path> = workspace
            .projects
            .iter()
            .map(|project| project.root.as_path())
            .collect();
        let blocked = lifecycle::run_install_scripts_for_projects(config, Some(workspace), &roots)?;

        for project in &workspace.projects {
            lifecycle::run_project_scripts(config, Some(workspace), &project.root)?;
        }

        blocked
    } else {
        Vec::new()
    };

    let workspace_integrity = build_workspace_integrity_state(workspace, &graph)?;
    write_workspace_integrity(&workspace.root, &workspace_integrity)?;

    for project in &workspace.projects {
        let project_integrity = super::utils::IntegrityState {
            lockfile_hash: workspace_integrity.lockfile_hash.clone(),
            patch_hash: compute_project_patch_hash(project)?,
        };
        write_integrity_path(&project.root.join("node_modules"), &project_integrity)?;
    }

    if include_dev {
        console::step("Saved lockfile");
    }

    console::clear_steps(if include_dev { 4 } else { 3 });

    let elapsed = started.elapsed();
    let seconds = elapsed.as_secs_f32();
    let package_count = graph.packages.len();

    console::summary(package_count, seconds);

    if !blocked_scripts.is_empty() {
        println!();
        console::blocked_scripts(&blocked_scripts);
    }

    Ok(InstallResult {
        package_count,
        elapsed_seconds: seconds,
    })
}

async fn resolve_workspace_deps(
    config: &SnpmConfig,
    client: &reqwest::Client,
    root_deps: &BTreeMap<String, String>,
    root_protocols: &BTreeMap<String, RegistryProtocol>,
    optional_root_names: &BTreeSet<String>,
    force: bool,
    store_paths: &mut BTreeMap<PackageId, PathBuf>,
) -> Result<ResolutionGraph> {
    use futures::lock::Mutex;

    let paths = Arc::new(Mutex::new(BTreeMap::new()));
    let tasks: Arc<Mutex<Vec<JoinHandle<Result<()>>>>> = Arc::new(Mutex::new(Vec::new()));
    let store_semaphore = Arc::new(tokio::sync::Semaphore::new(config.registry_concurrency));
    let progress_count = Arc::new(AtomicUsize::new(0));
    let progress_total = Arc::new(AtomicUsize::new(root_deps.len()));

    let min_age = config.min_package_age_days;

    let graph = {
        let paths = paths.clone();
        let tasks = tasks.clone();
        let sem = store_semaphore.clone();
        let count = progress_count.clone();
        let total = progress_total.clone();
        let config_clone = config.clone();
        let client_clone = client.clone();

        resolve::resolve_with_optional_roots(
            config,
            client,
            root_deps,
            root_protocols,
            optional_root_names,
            min_age,
            force,
            None,
            move |package| {
                let config = config_clone.clone();
                let client = client_clone.clone();
                let paths = paths.clone();
                let tasks = tasks.clone();
                let sem = sem.clone();
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
                        let _permit = sem.acquire().await.unwrap();
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
        let result = handle.await.map_err(|e| SnpmError::StoreTask {
            reason: e.to_string(),
        })?;
        result?;
    }

    *store_paths = paths.lock().await.clone();
    Ok(graph)
}

fn detect_workspace_scenario_early(
    workspace: &Workspace,
    lockfile_path: &Path,
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

    let graph = lockfile::to_graph(&existing);
    let integrity_state = match build_workspace_integrity_state(workspace, &graph) {
        Ok(state) => state,
        Err(_) => return (InstallScenario::Cold, Some(existing)),
    };

    if !force && check_workspace_integrity(&workspace.root, &integrity_state) {
        return (InstallScenario::Hot, Some(existing));
    }

    let cache_check = check_store_cache(config, &graph);
    if cache_check.missing.is_empty() {
        return (InstallScenario::WarmLinkOnly, Some(existing));
    }

    (InstallScenario::WarmPartialCache, Some(existing))
}

fn validate_lockfile_matches_manifest(
    scenario: InstallScenario,
    lockfile: Option<lockfile::Lockfile>,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
) -> (InstallScenario, Option<lockfile::Lockfile>) {
    if let Some(ref existing) = lockfile
        && !lockfile::root_specs_match(existing, required_root, optional_root)
    {
        return (InstallScenario::Cold, lockfile);
    }

    (scenario, lockfile)
}

fn rebuild_virtual_store_paths(
    virtual_store_dir: &Path,
    graph: &ResolutionGraph,
) -> Result<BTreeMap<PackageId, PathBuf>> {
    let mut paths = BTreeMap::new();

    for id in graph.packages.keys() {
        let safe_name = id.name.replace('/', "+");
        let virtual_id_dir = virtual_store_dir.join(format!("{}@{}", safe_name, id.version));
        let package_location = virtual_id_dir.join("node_modules").join(&id.name);
        paths.insert(id.clone(), package_location);
    }

    Ok(paths)
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

    packages.par_iter().try_for_each(|(id, _)| -> Result<()> {
        let safe_name = id.name.replace('/', "+");
        let virtual_id_dir = virtual_store_dir.join(format!("{}@{}", safe_name, id.version));
        let package_location = virtual_id_dir.join("node_modules").join(&id.name);
        let marker_file = virtual_id_dir.join(".snpm_linked");

        let store_path = store_paths.get(id).ok_or_else(|| SnpmError::StoreMissing {
            name: id.name.clone(),
            version: id.version.clone(),
        })?;

        if marker_file.is_file() {
            let is_real_dir = package_location
                .symlink_metadata()
                .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink());
            if is_real_dir
                && fs::read_dir(&package_location)
                    .ok()
                    .and_then(|mut d| d.next())
                    .is_some()
            {
                virtual_store_paths
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert((*id).clone(), package_location);
                return Ok(());
            }
            fs::remove_file(&marker_file).ok();
        }

        fs::remove_file(&package_location).ok();
        fs::remove_dir_all(&package_location).ok();

        crate::linker::fs::ensure_parent_dir(&package_location)?;

        crate::linker::fs::link_dir(config, store_path, &package_location)?;

        fs::write(&marker_file, []).map_err(|source| SnpmError::WriteFile {
            path: marker_file,
            source,
        })?;

        virtual_store_paths
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert((*id).clone(), package_location);

        Ok(())
    })?;

    let mutex = Arc::try_unwrap(virtual_store_paths).map_err(|_| SnpmError::Internal {
        reason: "virtual store paths Arc still has multiple owners".into(),
    })?;
    Ok(mutex.into_inner().unwrap_or_else(|e| e.into_inner()))
}

fn link_store_dependencies(
    virtual_store_paths: &BTreeMap<PackageId, PathBuf>,
    graph: &ResolutionGraph,
) -> Result<()> {
    let packages: Vec<_> = graph.packages.iter().collect();

    packages.par_iter().try_for_each(|(id, package)| {
        let package_location =
            virtual_store_paths
                .get(id)
                .ok_or_else(|| SnpmError::GraphMissing {
                    name: id.name.clone(),
                    version: id.version.clone(),
                })?;
        let package_node_modules =
            crate::linker::fs::package_node_modules(package_location, &id.name).ok_or_else(
                || SnpmError::GraphMissing {
                    name: id.name.clone(),
                    version: id.version.clone(),
                },
            )?;

        for (dep_name, dep_id) in &package.dependencies {
            let dep_target =
                virtual_store_paths
                    .get(dep_id)
                    .ok_or_else(|| SnpmError::GraphMissing {
                        name: dep_id.name.clone(),
                        version: dep_id.version.clone(),
                    })?;
            let dep_link = package_node_modules.join(dep_name);

            if crate::linker::fs::symlink_is_correct(&dep_link, dep_target) {
                continue;
            }

            fs::remove_file(&dep_link).ok();
            fs::remove_dir_all(&dep_link).ok();

            crate::linker::fs::ensure_parent_dir(&dep_link)?;

            symlink_dir_entry(dep_target, &dep_link)
                .or_else(|_| crate::linker::fs::copy_dir(dep_target, &dep_link))?;
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

    let (workspace_deps, workspace_dev_deps, workspace_optional_deps) =
        collect_workspace_protocol_deps(project);

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

    link_external_deps(
        &project.manifest.optional_dependencies,
        &workspace_optional_deps,
        graph,
        virtual_store_paths,
        &node_modules,
    )?;

    link_local_workspace_deps(
        project,
        Some(workspace),
        &workspace_deps,
        &workspace_dev_deps,
        &workspace_optional_deps,
        include_dev,
    )
}

fn collect_workspace_protocol_deps(
    project: &Project,
) -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
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

    let optional_deps = project
        .manifest
        .optional_dependencies
        .iter()
        .filter(|(_, v)| v.starts_with("workspace:"))
        .map(|(k, _)| k.clone())
        .collect();

    (deps, dev_deps, optional_deps)
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

fn apply_workspace_patches(
    workspace: &Workspace,
    store_paths: &BTreeMap<PackageId, PathBuf>,
) -> Result<usize> {
    let mut patches_to_apply = BTreeMap::<(String, String), (PathBuf, PathBuf)>::new();

    for project in &workspace.projects {
        for (name, version, patch_path) in get_patches_to_apply(project)? {
            let safe_name = name.replace('/', "+");
            let package_dir = project
                .root
                .join("node_modules")
                .join(".snpm")
                .join(format!("{}@{}", safe_name, version))
                .join("node_modules")
                .join(&name);

            if !package_dir.exists() {
                console::warn(&format!(
                    "Patch for {}@{} skipped: package not installed in {}",
                    name,
                    version,
                    project.root.display()
                ));
                continue;
            }

            let key = (name.clone(), version.clone());
            if let Some((existing_patch, _)) = patches_to_apply.get(&key) {
                if existing_patch != &patch_path {
                    return Err(SnpmError::WorkspaceConfig {
                        path: workspace.root.clone(),
                        reason: format!(
                            "conflicting patches configured for {}@{} across workspace projects",
                            name, version
                        ),
                    });
                }
                continue;
            }

            patches_to_apply.insert(key, (patch_path, package_dir));
        }
    }

    let mut applied = 0;

    for ((name, version), (patch_path, package_dir)) in patches_to_apply {
        let package_id = PackageId {
            name: name.clone(),
            version: version.clone(),
        };
        let Some(store_path) = store_paths.get(&package_id) else {
            console::warn(&format!(
                "Patch for {}@{} skipped: package missing from cache graph",
                name, version
            ));
            continue;
        };

        patch::materialize_patch_target(&package_dir, store_path)?;

        match patch::apply_patch(&package_dir, &patch_path) {
            Ok(()) => applied += 1,
            Err(error) => {
                console::warn(&format!(
                    "Failed to apply patch for {}@{}: {}",
                    name, version, error
                ));
            }
        }
    }

    Ok(applied)
}

fn check_workspace_integrity(workspace_root: &Path, state: &super::utils::IntegrityState) -> bool {
    check_integrity_path(&workspace_root.join("node_modules"), state)
}

fn write_workspace_integrity(
    workspace_root: &Path,
    state: &super::utils::IntegrityState,
) -> Result<()> {
    write_integrity_path(&workspace_root.join("node_modules"), state)
}

pub fn collect_workspace_root_deps(
    workspace: &Workspace,
    include_dev: bool,
) -> Result<BTreeMap<String, String>> {
    let root_specs = collect_workspace_root_specs(workspace, include_dev)?;
    let mut combined = root_specs.required;

    for (name, range) in root_specs.optional {
        combined.entry(name).or_insert(range);
    }

    Ok(combined)
}

pub fn collect_workspace_root_specs(
    workspace: &Workspace,
    include_dev: bool,
) -> Result<RootSpecSet> {
    let mut required = BTreeMap::new();
    let mut optional = BTreeMap::new();

    for member in workspace.projects.iter() {
        let mut local = BTreeSet::new();
        let dependencies = apply_specs(
            &member.manifest.dependencies,
            Some(workspace),
            None,
            &mut local,
            None,
        )?;
        let mut local_optional = BTreeSet::new();
        let optional_dependencies = apply_specs(
            &member.manifest.optional_dependencies,
            Some(workspace),
            None,
            &mut local_optional,
            None,
        )?;

        let development_dependencies = if include_dev {
            let mut local_development = BTreeSet::new();
            apply_specs(
                &member.manifest.dev_dependencies,
                Some(workspace),
                None,
                &mut local_development,
                None,
            )?
        } else {
            BTreeMap::new()
        };

        let member_specs = build_project_root_specs(
            &dependencies,
            &development_dependencies,
            &optional_dependencies,
            include_dev,
        );

        for (name, range) in member_specs.required.iter() {
            insert_workspace_root_dep(&mut required, &workspace.root, &member.root, name, range)?;
        }

        for (name, range) in member_specs.optional.iter() {
            if let Some(existing) = required.get(name) {
                if existing != range {
                    return Err(SnpmError::WorkspaceConfig {
                        path: workspace.root.clone(),
                        reason: format!(
                            "dependency {name} has conflicting ranges {existing} and {range} across workspace projects"
                        ),
                    });
                }
                continue;
            }

            insert_workspace_root_dep(&mut optional, &workspace.root, &member.root, name, range)?;
        }
    }

    Ok(RootSpecSet { required, optional })
}

pub fn insert_workspace_root_dep(
    combined: &mut BTreeMap<String, String>,
    workspace_root: &Path,
    declaring_package_root: &Path,
    name: &str,
    range: &str,
) -> Result<()> {
    let resolved_range = if let Some(file_path) = range.strip_prefix("file:") {
        let path = Path::new(file_path);
        if path.is_relative() {
            let absolute = declaring_package_root.join(path);
            let canonical = absolute
                .canonicalize()
                .map_err(|e| SnpmError::ResolutionFailed {
                    name: name.to_string(),
                    range: range.to_string(),
                    reason: format!("Failed to resolve file path {}: {}", absolute.display(), e),
                })?;
            format!("file:{}", canonical.display())
        } else {
            range.to_string()
        }
    } else {
        range.to_string()
    };

    if let Some(existing) = combined.get(name) {
        if existing != &resolved_range {
            return Err(SnpmError::WorkspaceConfig {
                path: workspace_root.to_path_buf(),
                reason: format!(
                    "dependency {name} has conflicting ranges {existing} and {resolved_range} across workspace projects"
                ),
            });
        }
    } else {
        combined.insert(name.to_string(), resolved_range);
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

    let version_parsed =
        snpm_semver::parse_version(version).map_err(|error| SnpmError::Semver {
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
    local_optional_deps: &BTreeSet<String>,
    include_dev: bool,
) -> Result<()> {
    if local_deps.is_empty() && local_dev_deps.is_empty() && local_optional_deps.is_empty() {
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

    for name in local_deps
        .iter()
        .chain(local_dev_deps.iter())
        .chain(local_optional_deps.iter())
    {
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

        crate::linker::fs::ensure_parent_dir(&dest)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Manifest;
    use crate::resolve::ResolvedPackage;
    use crate::workspace::types::WorkspaceConfig;
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    fn make_workspace_with_project(name: &str, version: &str) -> Workspace {
        let dir = std::env::temp_dir().join(format!("snpm_test_ws_{}", std::process::id()));
        let project = Project {
            root: dir.join(name),
            manifest_path: dir.join(name).join("package.json"),
            manifest: Manifest {
                name: Some(name.to_string()),
                version: Some(version.to_string()),
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
                optional_dependencies: BTreeMap::new(),
                scripts: BTreeMap::new(),
                files: None,
                bin: None,
                main: None,
                pnpm: None,
                snpm: None,
                workspaces: None,
            },
        };
        Workspace {
            root: dir,
            projects: vec![project],
            config: WorkspaceConfig {
                packages: Vec::new(),
                catalog: BTreeMap::new(),
                catalogs: BTreeMap::new(),
                only_built_dependencies: Vec::new(),
                ignored_built_dependencies: Vec::new(),
                hoisting: None,
            },
        }
    }

    #[test]
    fn validate_workspace_spec_wildcard() {
        let ws = make_workspace_with_project("my-lib", "1.0.0");
        assert!(validate_workspace_spec(&ws, "my-lib", "workspace:*").is_ok());
    }

    #[test]
    fn validate_workspace_spec_empty() {
        let ws = make_workspace_with_project("my-lib", "1.0.0");
        assert!(validate_workspace_spec(&ws, "my-lib", "workspace:").is_ok());
    }

    #[test]
    fn validate_workspace_spec_caret() {
        let ws = make_workspace_with_project("my-lib", "1.2.3");
        assert!(validate_workspace_spec(&ws, "my-lib", "workspace:^").is_ok());
    }

    #[test]
    fn validate_workspace_spec_tilde() {
        let ws = make_workspace_with_project("my-lib", "1.2.3");
        assert!(validate_workspace_spec(&ws, "my-lib", "workspace:~").is_ok());
    }

    #[test]
    fn validate_workspace_spec_explicit_range_satisfied() {
        let ws = make_workspace_with_project("my-lib", "1.2.3");
        assert!(validate_workspace_spec(&ws, "my-lib", "workspace:^1.0.0").is_ok());
    }

    #[test]
    fn validate_workspace_spec_explicit_range_not_satisfied() {
        let ws = make_workspace_with_project("my-lib", "1.2.3");
        assert!(validate_workspace_spec(&ws, "my-lib", "workspace:^2.0.0").is_err());
    }

    #[test]
    fn validate_workspace_spec_missing_project() {
        let ws = make_workspace_with_project("my-lib", "1.0.0");
        assert!(validate_workspace_spec(&ws, "nonexistent", "workspace:*").is_err());
    }

    #[test]
    fn collect_workspace_protocol_deps_filters_correctly() {
        let project = Project {
            root: PathBuf::from("/tmp/project"),
            manifest_path: PathBuf::from("/tmp/project/package.json"),
            manifest: Manifest {
                name: Some("test".to_string()),
                version: None,
                dependencies: BTreeMap::from([
                    ("lib-a".to_string(), "workspace:*".to_string()),
                    ("lodash".to_string(), "^4.0.0".to_string()),
                ]),
                dev_dependencies: BTreeMap::from([
                    ("lib-b".to_string(), "workspace:^".to_string()),
                    ("jest".to_string(), "^29.0.0".to_string()),
                ]),
                optional_dependencies: BTreeMap::from([(
                    "lib-c".to_string(),
                    "workspace:~".to_string(),
                )]),
                scripts: BTreeMap::new(),
                files: None,
                bin: None,
                main: None,
                pnpm: None,
                snpm: None,
                workspaces: None,
            },
        };

        let (deps, dev_deps, optional_deps) = collect_workspace_protocol_deps(&project);
        assert!(deps.contains("lib-a"));
        assert!(!deps.contains("lodash"));
        assert!(dev_deps.contains("lib-b"));
        assert!(!dev_deps.contains("jest"));
        assert!(optional_deps.contains("lib-c"));
    }

    #[test]
    fn validate_lockfile_matches_returns_cold_on_mismatch() {
        let lockfile = lockfile::Lockfile {
            version: 1,
            root: lockfile::LockRoot {
                dependencies: BTreeMap::from([(
                    "a".to_string(),
                    lockfile::LockRootDependency {
                        requested: "^1.0.0".to_string(),
                        version: Some("1.0.0".to_string()),
                        optional: false,
                    },
                )]),
            },
            packages: BTreeMap::new(),
        };

        let required = BTreeMap::from([("b".to_string(), "^2.0.0".to_string())]);
        let (scenario, _) = validate_lockfile_matches_manifest(
            InstallScenario::WarmLinkOnly,
            Some(lockfile),
            &required,
            &BTreeMap::new(),
        );
        assert_eq!(scenario, InstallScenario::Cold);
    }

    #[test]
    fn validate_lockfile_matches_preserves_scenario_on_match() {
        let lockfile = lockfile::Lockfile {
            version: 1,
            root: lockfile::LockRoot {
                dependencies: BTreeMap::from([(
                    "a".to_string(),
                    lockfile::LockRootDependency {
                        requested: "^1.0.0".to_string(),
                        version: Some("1.0.0".to_string()),
                        optional: false,
                    },
                )]),
            },
            packages: BTreeMap::new(),
        };

        let required = BTreeMap::from([("a".to_string(), "^1.0.0".to_string())]);
        let (scenario, _) = validate_lockfile_matches_manifest(
            InstallScenario::WarmLinkOnly,
            Some(lockfile),
            &required,
            &BTreeMap::new(),
        );
        assert_eq!(scenario, InstallScenario::WarmLinkOnly);
    }

    #[test]
    fn rebuild_virtual_store_paths_scoped_package() {
        let dir = tempdir().unwrap();
        let store_dir = dir.path().join(".snpm");

        let id = PackageId {
            name: "@scope/pkg".to_string(),
            version: "1.0.0".to_string(),
        };
        let pkg = ResolvedPackage {
            id: id.clone(),
            tarball: String::new(),
            integrity: None,
            dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: None,
            has_bin: false,
        };
        let graph = ResolutionGraph {
            root: resolve::ResolutionRoot {
                dependencies: BTreeMap::new(),
            },
            packages: BTreeMap::from([(id.clone(), pkg)]),
        };

        let paths = rebuild_virtual_store_paths(&store_dir, &graph).unwrap();
        let path = paths.get(&id).unwrap();
        // Should use + for scoped names
        assert!(path.to_string_lossy().contains("@scope+pkg@1.0.0"));
        assert!(path.to_string_lossy().contains("node_modules/@scope/pkg"));
    }
}
