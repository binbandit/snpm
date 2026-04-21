use crate::console;
use crate::resolve::{self, PackageId, ResolutionGraph};
use crate::store;
use crate::{Result, SnpmConfig, SnpmError};

use futures::lock::Mutex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::task::JoinHandle;

pub(crate) async fn resolve_workspace_deps(
    config: &SnpmConfig,
    client: &reqwest::Client,
    root_deps: &BTreeMap<String, String>,
    root_protocols: &BTreeMap<String, crate::registry::RegistryProtocol>,
    optional_root_names: &BTreeSet<String>,
    force: bool,
    overrides: Option<&BTreeMap<String, String>>,
    workspace_sources: Option<&BTreeMap<String, String>>,
    existing_graph: Option<&ResolutionGraph>,
    store_paths: &mut BTreeMap<PackageId, PathBuf>,
) -> Result<ResolutionGraph> {
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

        if let Some(seed_graph) = existing_graph {
            resolve::resolve_with_optional_roots_with_seed(
                config,
                client,
                root_deps,
                root_protocols,
                optional_root_names,
                min_age,
                force,
                overrides,
                workspace_sources,
                Some(seed_graph),
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
            .await?
        } else {
            resolve::resolve_with_optional_roots(
                config,
                client,
                root_deps,
                root_protocols,
                optional_root_names,
                min_age,
                force,
                overrides,
                workspace_sources,
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
            .await?
        }
    };

    await_store_tasks(&tasks).await?;
    *store_paths = paths.lock().await.clone();
    Ok(graph)
}

async fn await_store_tasks(tasks: &Arc<Mutex<Vec<JoinHandle<Result<()>>>>>) -> Result<()> {
    let handles = {
        let mut guard = tasks.lock().await;
        std::mem::take(&mut *guard)
    };

    for handle in handles {
        let result = handle.await.map_err(|error| SnpmError::StoreTask {
            reason: error.to_string(),
        })?;
        result?;
    }

    Ok(())
}
