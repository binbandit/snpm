use crate::console;
use crate::lifecycle;
use crate::linker;
use crate::lockfile;
use crate::patch;
use crate::registry::RegistryProtocol;
use crate::resolve;
use crate::store;
use crate::workspace::{CatalogConfig, OverridesConfig};
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace};

use futures::lock::Mutex;
use reqwest::Client;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tokio::task::JoinHandle;

pub mod manifest;
pub mod utils;
pub mod workspace;

pub use manifest::*;
pub use utils::*;
pub use workspace::*;

pub async fn install(
    config: &SnpmConfig,
    project: &mut Project,
    options: InstallOptions,
) -> Result<InstallResult> {
    let started = Instant::now();

    console::verbose(&format!(
        "install start: root={} requested=[{}] dev={} include_dev={} frozen_lockfile={} force={}",
        project.root.display(),
        options.requested.join(", "),
        options.dev,
        options.include_dev,
        options.frozen_lockfile,
        options.force,
    ));

    let registry_client = Client::new();

    let (requested_ranges_raw, requested_protocols_raw) =
        parse_requested_with_protocol(&options.requested);

    let mut additions = BTreeMap::new();
    let mut requested_protocols = BTreeMap::new();

    for (name, range) in requested_ranges_raw {
        if project.manifest.dependencies.contains_key(&name)
            || project.manifest.dev_dependencies.contains_key(&name)
        {
            continue;
        }

        additions.insert(name.clone(), range);
        if let Some(protocol) = requested_protocols_raw.get(&name) {
            requested_protocols.insert(name.clone(), protocol.clone());
        }
    }

    let workspace = Workspace::discover(&project.root)?;

    let catalog = if workspace.is_none() {
        CatalogConfig::load(&project.root)?
    } else {
        None
    };

    let overrides_from_file = if let Some(workspace_reference) = workspace.as_ref() {
        OverridesConfig::load(&workspace_reference.root)?
            .map(|config| config.overrides)
            .unwrap_or_default()
    } else {
        OverridesConfig::load(&project.root)?
            .map(|c| c.overrides)
            .unwrap_or_default()
    };

    let mut overrides = overrides_from_file;

    if let Some(pnpm) = &project.manifest.pnpm {
        for (name, range) in pnpm.overrides.iter() {
            overrides.insert(name.clone(), range.clone());
        }
    }

    if let Some(snpm) = &project.manifest.snpm {
        for (name, range) in snpm.overrides.iter() {
            overrides.insert(name.clone(), range.clone());
        }
    }

    let workspace_root = workspace
        .as_ref()
        .map(|workspace| format!("{}", workspace.root.display()))
        .unwrap_or_else(|| "<none>".to_string());
    console::verbose(&format!(
        "workspace_root={} overrides={} catalog_local={}",
        workspace_root,
        overrides.len(),
        catalog.as_ref().map(|c| c.catalog.len()).unwrap_or(0),
    ));

    let mut local_deps = BTreeSet::new();
    let mut local_dev_deps = BTreeSet::new();
    let mut manifest_protocols = BTreeMap::new();

    let dependencies = apply_specs(
        &project.manifest.dependencies,
        workspace.as_ref(),
        catalog.as_ref(),
        &mut local_deps,
        Some(&mut manifest_protocols),
    )?;
    let development_dependencies = apply_specs(
        &project.manifest.dev_dependencies,
        workspace.as_ref(),
        catalog.as_ref(),
        &mut local_dev_deps,
        Some(&mut manifest_protocols),
    )?;

    let manifest_root = if let Some(workspace) = workspace.as_ref() {
        collect_workspace_root_deps(workspace, options.include_dev)?
    } else {
        build_project_manifest_root(
            &dependencies,
            &development_dependencies,
            options.include_dev,
        )
    };

    let mut root_dependencies = manifest_root.clone();

    for (name, range) in additions.iter() {
        root_dependencies.insert(name.clone(), range.clone());
    }

    console::verbose(&format!(
        "manifest_root_deps={} root_deps={} additions={}",
        manifest_root.len(),
        root_dependencies.len(),
        additions.len()
    ));

    let mut root_protocols = BTreeMap::new();

    for name in manifest_root.keys() {
        if let Some(protocol) = manifest_protocols.get(name) {
            root_protocols.insert(name.clone(), protocol.clone());
        } else {
            root_protocols.insert(name.clone(), RegistryProtocol::npm());
        }
    }

    for name in additions.keys() {
        if let Some(protocol) = requested_protocols.get(name) {
            root_protocols.insert(name.clone(), protocol.clone());
        } else {
            root_protocols
                .entry(name.clone())
                .or_insert_with(RegistryProtocol::npm);
        }
    }

    let lockfile_path = workspace
        .as_ref()
        .map(|w| w.root.join("snpm-lock.yaml"))
        .unwrap_or_else(|| project.root.join("snpm-lock.yaml"));

    let is_fresh_install = !lockfile_path.exists();

    console::verbose(&format!(
        "lockfile_path={} exists={} fresh_install={}",
        lockfile_path.display(),
        lockfile_path.is_file(),
        is_fresh_install
    ));

    if options.frozen_lockfile || config.frozen_lockfile_default {
        let source = if config.frozen_lockfile_default {
            "SNPM_FROZEN_LOCKFILE=1"
        } else {
            "--frozen-lockfile"
        };
        console::verbose(&format!(
            "using frozen lockfile at {} (source: {})",
            lockfile_path.display(),
            source
        ));

        if !lockfile_path.is_file() {
            return Err(SnpmError::Lockfile {
                path: lockfile_path.clone(),
                reason: "frozen-lockfile requested but snpm-lock.yaml is missing".into(),
            });
        }

        if !additions.is_empty() {
            return Err(SnpmError::Lockfile {
                path: lockfile_path.clone(),
                reason: "cannot add package when using frozen-lockfile".into(),
            });
        }

        let existing = lockfile::read(&lockfile_path)?;
        let mut lock_requested = BTreeMap::new();

        for (name, dep) in existing.root.dependencies.iter() {
            lock_requested.insert(name.clone(), dep.requested.clone());
        }

        if lock_requested != manifest_root {
            return Err(SnpmError::Lockfile {
                path: lockfile_path.clone(),
                reason:
                    "manifest dependencies do not match snpm-lock.yaml when using frozen-lockfile"
                        .into(),
            });
        }
    }

    let can_use_scenario_optimization = options.include_dev && additions.is_empty();

    let (scenario, existing_lockfile) = if can_use_scenario_optimization {
        detect_install_scenario(
            project,
            &lockfile_path,
            &manifest_root,
            config,
            options.force,
        )
    } else {
        (InstallScenario::Cold, None)
    };

    let mut lockfile_reused_unchanged = false;
    let mut early_exit = false;
    let mut store_paths_map: BTreeMap<crate::resolve::PackageId, std::path::PathBuf> =
        BTreeMap::new();

    let graph = match scenario {
        InstallScenario::Hot => {
            early_exit = true;
            lockfile_reused_unchanged = true;
            let existing = existing_lockfile.expect("Hot scenario requires lockfile");
            lockfile::to_graph(&existing)
        }

        InstallScenario::WarmLinkOnly => {
            lockfile_reused_unchanged = true;
            let existing = existing_lockfile.expect("WarmLinkOnly scenario requires lockfile");
            let graph = lockfile::to_graph(&existing);

            let cache_check = check_store_cache(config, &graph);
            store_paths_map = cache_check.cached;

            console::verbose(&format!(
                "warm link-only path: {} packages from cache",
                store_paths_map.len()
            ));

            console::step_with_count("Using cached packages", store_paths_map.len());
            graph
        }

        InstallScenario::WarmPartialCache => {
            lockfile_reused_unchanged = true;
            let existing = existing_lockfile.expect("WarmPartialCache scenario requires lockfile");
            let graph = lockfile::to_graph(&existing);

            let cache_check = check_store_cache(config, &graph);
            let cached_count = cache_check.cached.len();
            let missing_count = cache_check.missing.len();

            console::verbose(&format!(
                "warm partial-cache path: {} cached, {} to download",
                cached_count, missing_count
            ));

            store_paths_map = cache_check.cached;

            if !cache_check.missing.is_empty() {
                console::step("Downloading missing packages");
                let materialize_start = Instant::now();
                let downloaded = materialize_missing_packages(config, &cache_check.missing).await?;

                console::verbose(&format!(
                    "downloaded {} missing packages in {:.3}s",
                    downloaded.len(),
                    materialize_start.elapsed().as_secs_f64()
                ));

                store_paths_map.extend(downloaded);
            }

            console::step_with_count("Resolved, downloaded and extracted", store_paths_map.len());
            graph
        }

        InstallScenario::Cold => {
            console::step("Resolving dependencies");
            console::verbose("cold path: full resolution required");

            let store_paths = Arc::new(Mutex::new(BTreeMap::<
                crate::resolve::PackageId,
                std::path::PathBuf,
            >::new()));
            let store_config = config.clone();
            let store_client = registry_client.clone();
            let store_tasks: Arc<Mutex<Vec<JoinHandle<Result<()>>>>> =
                Arc::new(Mutex::new(Vec::new()));

            let resolve_started = Instant::now();
            let paths = store_paths.clone();
            let config_clone = store_config.clone();
            let client = store_client.clone();
            let tasks = store_tasks.clone();
            let progress_count = Arc::new(AtomicUsize::new(0));
            let progress_total = Arc::new(AtomicUsize::new(root_dependencies.len()));

            let graph = resolve::resolve(
                config,
                &registry_client,
                &root_dependencies,
                &root_protocols,
                config.min_package_age_days,
                options.force,
                Some(&overrides),
                move |package| {
                    let config = config_clone.clone();
                    let client = client.clone();
                    let paths = paths.clone();
                    let tasks = tasks.clone();
                    let count = progress_count.clone();
                    let total = progress_total.clone();
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
                            let path = store::ensure_package(&config, &package, &client).await?;
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
            .await?;

            console::verbose(&format!(
                "resolve completed in {:.3}s (packages={})",
                resolve_started.elapsed().as_secs_f64(),
                graph.packages.len()
            ));

            {
                let handles = {
                    let mut guard = store_tasks.lock().await;
                    std::mem::take(&mut *guard)
                };

                console::verbose(&format!("joining {} store tasks", handles.len()));

                let store_wait_started = Instant::now();

                for handle in handles {
                    let result = handle.await.map_err(|error| SnpmError::StoreTask {
                        reason: error.to_string(),
                    })?;
                    result?;
                }

                console::verbose(&format!(
                    "store tasks completed in {:.3}s",
                    store_wait_started.elapsed().as_secs_f64()
                ));
            }

            store_paths_map = {
                let guard = store_paths.lock().await;
                guard.clone()
            };

            if store_paths_map.is_empty() && !graph.packages.is_empty() {
                console::verbose("no store tasks were scheduled; materializing store on demand");
                let materialize_start = Instant::now();
                store_paths_map = materialize_store(config, &graph).await?;
                console::verbose(&format!(
                    "materialized store in {:.3}s (packages={})",
                    materialize_start.elapsed().as_secs_f64(),
                    store_paths_map.len()
                ));
            }

            if options.include_dev {
                lockfile::write(&lockfile_path, &graph)?;
            }

            console::step_with_count("Resolved, downloaded and extracted", store_paths_map.len());
            graph
        }
    };

    write_manifest(
        project,
        &graph,
        &additions,
        options.dev,
        workspace.as_ref(),
        catalog.as_ref(),
    )?;

    let mut should_link = true;

    if early_exit {
        should_link = false;
        console::verbose("using early exit path (warm path optimization)");
    } else if lockfile_reused_unchanged && !options.force {
        console::verbose("lockfile reused without changes; checking existing node_modules");

        let lockfile_hash = compute_lockfile_hash(&graph);
        if check_integrity_file(project, &lockfile_hash) {
            should_link = false;
            early_exit = true;

            console::verbose(
                "node_modules is up to date; taking early exit path (warm path optimization)",
            );
        }
    }

    if should_link {
        let link_start = Instant::now();
        linker::link(
            config,
            workspace.as_ref(),
            project,
            &graph,
            &store_paths_map,
            options.include_dev,
        )?;

        link_local_workspace_deps(
            project,
            workspace.as_ref(),
            &local_deps,
            &local_dev_deps,
            options.include_dev,
        )?;

        let patches_applied = apply_patches(project)?;
        if patches_applied > 0 {
            console::verbose(&format!("applied {} patches", patches_applied));
        }

        let lockfile_hash = compute_lockfile_hash(&graph);
        write_integrity_file(project, &lockfile_hash)?;

        console::verbose(&format!(
            "linking completed in {:.3}s",
            link_start.elapsed().as_secs_f64()
        ));
    }

    if options.include_dev {
        console::step("Saved lockfile");
    }

    let scripts_start = Instant::now();
    let blocked_scripts = if early_exit {
        console::verbose("skipping install scripts (early exit - node_modules is fresh)");
        Vec::new()
    } else if can_any_scripts_run(config, workspace.as_ref()) {
        let blocked = lifecycle::run_install_scripts(config, workspace.as_ref(), &project.root)?;
        lifecycle::run_project_scripts(config, workspace.as_ref(), &project.root)?;

        blocked
    } else {
        console::verbose("skipping install scripts (no scripts can run based on config)");
        Vec::new()
    };

    let scripts_elapsed = scripts_start.elapsed();

    console::verbose(&format!(
        "install scripts completed in {:.3}s (blocked_scripts={})",
        scripts_elapsed.as_secs_f64(),
        blocked_scripts.len()
    ));

    let step_count = if options.include_dev { 3 } else { 2 };
    console::clear_steps(step_count);

    if !additions.is_empty() || is_fresh_install {
        println!();

        let mut packages_to_show: Vec<(String, String, bool)> = Vec::new();

        if !additions.is_empty() {
            for name in additions.keys() {
                if let Some(dep) = graph.root.dependencies.get(name) {
                    packages_to_show.push((
                        name.clone(),
                        dep.resolved.version.clone(),
                        options.dev,
                    ));
                }
            }
        } else if is_fresh_install {
            for (name, dep) in graph.root.dependencies.iter() {
                let is_dev = local_dev_deps.contains(name) && !local_deps.contains(name);
                packages_to_show.push((name.clone(), dep.resolved.version.clone(), is_dev));
            }
        }

        packages_to_show.sort_by(|a, b| a.0.cmp(&b.0));

        for (name, version, is_dev) in packages_to_show {
            console::added(&name, &version, is_dev);
        }
    }

    let elapsed = started.elapsed();
    let seconds = elapsed.as_secs_f32();
    let package_count = graph.packages.len();

    if !options.silent_summary {
        console::summary(package_count, seconds);
    }

    console::verbose(&format!(
        "install completed in {:.3}s (packages={} store_paths={} additions={} is_fresh_install={} blocked_scripts={})",
        seconds,
        package_count,
        store_paths_map.len(),
        additions.len(),
        is_fresh_install,
        blocked_scripts.len()
    ));

    if !blocked_scripts.is_empty() {
        println!();
        console::blocked_scripts(&blocked_scripts);
    }

    Ok(InstallResult {
        package_count,
        elapsed_seconds: seconds,
    })
}

pub async fn outdated(
    config: &SnpmConfig,
    project: &Project,
    include_dev: bool,
    force: bool,
) -> Result<Vec<OutdatedEntry>> {
    let workspace = Workspace::discover(&project.root)?;

    let registry_client = Client::new();

    let overrides = if let Some(ref workspace_reference) = workspace {
        match OverridesConfig::load(&workspace_reference.root)? {
            Some(config) => config.overrides,
            None => BTreeMap::new(),
        }
    } else {
        match OverridesConfig::load(&project.root)? {
            Some(config) => config.overrides,
            None => BTreeMap::new(),
        }
    };

    let catalog = if workspace.is_none() {
        CatalogConfig::load(&project.root)?
    } else {
        None
    };

    let mut local_deps = BTreeSet::new();
    let mut local_dev_deps = BTreeSet::new();
    let mut manifest_protocols = BTreeMap::new();

    let dependencies = apply_specs(
        &project.manifest.dependencies,
        workspace.as_ref(),
        catalog.as_ref(),
        &mut local_deps,
        Some(&mut manifest_protocols),
    )?;
    let development_dependencies = apply_specs(
        &project.manifest.dev_dependencies,
        workspace.as_ref(),
        catalog.as_ref(),
        &mut local_dev_deps,
        Some(&mut manifest_protocols),
    )?;

    let root_dependencies = if let Some(workspace_reference) = workspace.as_ref() {
        collect_workspace_root_deps(workspace_reference, include_dev)?
    } else {
        build_project_manifest_root(&dependencies, &development_dependencies, include_dev)
    };

    let mut root_protocols = BTreeMap::new();
    for name in root_dependencies.keys() {
        if let Some(protocol) = manifest_protocols.get(name) {
            root_protocols.insert(name.clone(), protocol.clone());
        } else {
            root_protocols.insert(name.clone(), RegistryProtocol::npm());
        }
    }

    console::verbose(&format!(
        "outdated: resolving {} root deps (include_dev={} force={})",
        root_dependencies.len(),
        include_dev,
        force
    ));

    let outdated_resolve_started = Instant::now();
    let graph = resolve::resolve(
        config,
        &registry_client,
        &root_dependencies,
        &root_protocols,
        config.min_package_age_days,
        force,
        Some(&overrides),
        |_package| async { Ok::<(), SnpmError>(()) },
    )
    .await?;
    console::verbose(&format!(
        "outdated: resolve completed in {:.3}s (packages={})",
        outdated_resolve_started.elapsed().as_secs_f64(),
        graph.packages.len()
    ));

    let lockfile_path = workspace
        .as_ref()
        .map(|w| w.root.join("snpm-lock.yaml"))
        .unwrap_or_else(|| project.root.join("snpm-lock.yaml"));

    let mut current_versions = BTreeMap::new();

    if lockfile_path.is_file() {
        let existing = lockfile::read(&lockfile_path)?;

        for (name, dep) in existing.root.dependencies.iter() {
            current_versions.insert(name.clone(), dep.version.clone());
        }
    }

    let mut names = BTreeSet::new();
    for name in dependencies.keys() {
        names.insert(name.clone());
    }
    if include_dev {
        for name in development_dependencies.keys() {
            names.insert(name.clone());
        }
    }

    let mut result = Vec::new();

    for name in names {
        let root_dep = match graph.root.dependencies.get(&name) {
            Some(dep) => dep,
            None => continue,
        };

        let wanted = root_dep.resolved.version.clone();
        let current = current_versions.get(&name).cloned();

        if let Some(ref current_version) = current
            && current_version == &wanted
        {
            continue;
        }

        result.push(OutdatedEntry {
            name,
            current,
            wanted,
        });
    }

    Ok(result)
}

pub async fn remove(config: &SnpmConfig, project: &mut Project, specs: Vec<String>) -> Result<()> {
    if specs.is_empty() {
        return Ok(());
    }

    let mut manifest = project.manifest.clone();

    for spec in specs {
        let (name, _) = parse_spec(&spec);

        let mut removed_any = false;

        if manifest.dependencies.remove(&name).is_some() {
            removed_any = true;
        }

        if manifest.dev_dependencies.remove(&name).is_some() {
            removed_any = true;
        }

        if removed_any {
            crate::console::removed(&name);
        }
    }

    project.write_manifest(&manifest)?;
    project.manifest = manifest;

    let options = InstallOptions {
        requested: Vec::new(),
        dev: false,
        include_dev: true,
        frozen_lockfile: false,
        force: false,
        silent_summary: false,
    };

    install(config, project, options).await?;
    Ok(())
}

pub async fn upgrade(
    config: &SnpmConfig,
    project: &mut Project,
    packages: Vec<String>,
    production: bool,
    force: bool,
) -> Result<()> {
    if packages.is_empty() {
        let options = InstallOptions {
            requested: Vec::new(),
            dev: false,
            include_dev: !production,
            frozen_lockfile: false,
            force,
            silent_summary: false,
        };

        install(config, project, options).await?;
        return Ok(());
    }

    let include_dev = !production;
    let entries = outdated(config, project, include_dev, force).await?;

    if entries.is_empty() {
        return Ok(());
    }

    let mut wanted_by_name = BTreeMap::new();

    for entry in entries {
        wanted_by_name.insert(entry.name, entry.wanted);
    }

    let mut manifest = project.manifest.clone();
    let mut changed = false;

    for spec in packages {
        let (name, _) = parse_spec(&spec);

        let wanted = match wanted_by_name.get(&name) {
            Some(version) => version,
            None => continue,
        };

        let mut updated = false;

        if let Some(current) = manifest.dependencies.get_mut(&name)
            && !is_special_protocol_spec(current)
        {
            *current = format!("^{}", wanted);
            console::info(&format!("updating {name} to ^{wanted}"));
            updated = true;
        }

        if !updated
            && !production
            && let Some(current) = manifest.dev_dependencies.get_mut(&name)
            && !is_special_protocol_spec(current)
        {
            *current = format!("^{}", wanted);
            console::info(&format!("updating {name} (dev) to ^{wanted}"));
            updated = true;
        }

        if updated {
            changed = true;
        }
    }

    if !changed {
        return Ok(());
    }

    project.write_manifest(&manifest)?;
    project.manifest = manifest;

    let options = InstallOptions {
        requested: Vec::new(),
        dev: false,
        include_dev,
        frozen_lockfile: false,
        force,
        silent_summary: false,
    };

    install(config, project, options).await?;
    Ok(())
}

fn apply_patches(project: &Project) -> Result<usize> {
    let patches = super::patch::get_patches_to_apply(project)?;

    if patches.is_empty() {
        return Ok(0);
    }

    let node_modules = project.root.join("node_modules");
    let mut applied = 0;

    for (name, version, patch_path) in &patches {
        let package_dir = node_modules.join(name);

        if !package_dir.exists() {
            console::warn(&format!(
                "Patch for {}@{} skipped: package not installed",
                name, version
            ));
            continue;
        }

        console::verbose(&format!(
            "applying patch for {}@{} from {}",
            name,
            version,
            patch_path.display()
        ));

        match patch::apply_patch(&package_dir, patch_path) {
            Ok(()) => {
                console::step(&format!("Applied patch for {}@{}", name, version));
                applied += 1;
            }
            Err(e) => {
                console::warn(&format!(
                    "Failed to apply patch for {}@{}: {}",
                    name, version, e
                ));
            }
        }
    }

    Ok(applied)
}
