use super::plan::ProjectInstallPlan;
use crate::console;
use crate::resolve::{self, PackageId, ResolutionGraph};
use crate::store;
use crate::{Result, SnpmConfig, SnpmError};

use futures::lock::Mutex;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use tokio::task::JoinHandle;

pub(super) async fn resolve_cold_install(
    config: &SnpmConfig,
    registry_client: &reqwest::Client,
    plan: &ProjectInstallPlan,
    root_dependencies: &BTreeMap<String, String>,
    force: bool,
    existing_graph: Option<&ResolutionGraph>,
) -> Result<(ResolutionGraph, BTreeMap<PackageId, PathBuf>)> {
    console::step("Resolving dependencies");
    console::verbose("cold path: full resolution required");

    let store_paths = Arc::new(Mutex::new(BTreeMap::<PackageId, PathBuf>::new()));
    let store_tasks: Arc<Mutex<Vec<JoinHandle<Result<()>>>>> = Arc::new(Mutex::new(Vec::new()));

    let resolve_started = Instant::now();
    let paths = store_paths.clone();
    let tasks = store_tasks.clone();
    let config_clone = config.clone();
    let client_clone = registry_client.clone();
    let progress_count = Arc::new(AtomicUsize::new(0));
    let progress_total = Arc::new(AtomicUsize::new(root_dependencies.len()));
    let workspace_sources = plan.workspace.as_ref().map(|workspace| {
        workspace
            .projects
            .iter()
            .filter_map(|project| {
                Some((
                    project.manifest.name.clone()?,
                    project.root.to_string_lossy().into_owned(),
                ))
            })
            .collect::<BTreeMap<_, _>>()
    });

    let graph = if let Some(seed_graph) = existing_graph {
        resolve::resolve_with_optional_roots_with_seed(
            config,
            registry_client,
            root_dependencies,
            &plan.root_protocols,
            &plan.optional_root_names,
            config.min_package_age_days,
            force,
            Some(&plan.overrides),
            workspace_sources.as_ref(),
            Some(seed_graph),
            move |package| {
                let config = config_clone.clone();
                let client = client_clone.clone();
                let paths = paths.clone();
                let tasks = tasks.clone();
                let count = progress_count.clone();
                let total = progress_total.clone();
                let name = package.id.name.clone();

                async move {
                    let current = count.fetch_add(1, Ordering::Relaxed) + 1;
                    let mut total_value = total.load(Ordering::Relaxed);
                    if current > total_value {
                        total_value = current;
                        total.store(total_value, Ordering::Relaxed);
                    }
                    console::progress("🚚", &name, current, total_value);

                    let package_id = package.id.clone();
                    let handle = tokio::spawn(async move {
                        let path = store::ensure_package(&config, &package, &client).await?;
                        let mut map = paths.lock().await;
                        map.insert(package_id, path);
                        Ok::<(), SnpmError>(())
                    });

                    tasks.lock().await.push(handle);
                    Ok(())
                }
            },
        )
        .await?
    } else {
        resolve::resolve_with_optional_roots(
            config,
            registry_client,
            root_dependencies,
            &plan.root_protocols,
            &plan.optional_root_names,
            config.min_package_age_days,
            force,
            Some(&plan.overrides),
            workspace_sources.as_ref(),
            move |package| {
                let config = config_clone.clone();
                let client = client_clone.clone();
                let paths = paths.clone();
                let tasks = tasks.clone();
                let count = progress_count.clone();
                let total = progress_total.clone();
                let name = package.id.name.clone();

                async move {
                    let current = count.fetch_add(1, Ordering::Relaxed) + 1;
                    let mut total_value = total.load(Ordering::Relaxed);
                    if current > total_value {
                        total_value = current;
                        total.store(total_value, Ordering::Relaxed);
                    }
                    console::progress("🚚", &name, current, total_value);

                    let package_id = package.id.clone();
                    let handle = tokio::spawn(async move {
                        let path = store::ensure_package(&config, &package, &client).await?;
                        let mut map = paths.lock().await;
                        map.insert(package_id, path);
                        Ok::<(), SnpmError>(())
                    });

                    tasks.lock().await.push(handle);
                    Ok(())
                }
            },
        )
        .await?
    };

    console::verbose(&format!(
        "resolve completed in {:.3}s (packages={})",
        resolve_started.elapsed().as_secs_f64(),
        graph.packages.len()
    ));

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

    let mut store_paths_map = match Arc::try_unwrap(store_paths) {
        Ok(mutex) => mutex.into_inner(),
        Err(arc) => arc.lock().await.clone(),
    };

    if store_paths_map.is_empty() && !graph.packages.is_empty() {
        console::verbose("no store tasks were scheduled; materializing store on demand");
        let materialize_start = Instant::now();
        store_paths_map =
            super::super::utils::materialize_store(config, &graph, registry_client).await?;
        console::verbose(&format!(
            "materialized store in {:.3}s (packages={})",
            materialize_start.elapsed().as_secs_f64(),
            store_paths_map.len()
        ));
    }

    Ok((graph, store_paths_map))
}
