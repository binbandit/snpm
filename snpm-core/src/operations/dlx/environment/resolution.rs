use crate::config::OfflineMode;
use crate::console;
use crate::resolve::{self, PackageId, ResolutionGraph};
use crate::{Result, SnpmConfig, SnpmError, store};

use futures::lock::Mutex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

pub(super) async fn resolve_dlx_graph(
    dlx_config: &SnpmConfig,
    registry_client: &reqwest::Client,
    root_deps: &BTreeMap<String, String>,
    offline_mode: OfflineMode,
    store_config: &SnpmConfig,
) -> Result<(ResolutionGraph, BTreeMap<PackageId, PathBuf>)> {
    let store_paths = Arc::new(Mutex::new(BTreeMap::new()));
    let store_tasks = Arc::new(Mutex::new(Vec::<JoinHandle<Result<()>>>::new()));
    let store_task_limit = Arc::new(Semaphore::new(store::store_task_concurrency(store_config)));
    let progress_count = Arc::new(AtomicUsize::new(0));
    let progress_total = Arc::new(AtomicUsize::new(1));

    let graph = resolve::resolve_with_offline(
        dlx_config,
        registry_client,
        root_deps,
        &BTreeMap::new(),
        &BTreeSet::new(),
        dlx_config.min_package_age_days,
        true,
        None,
        None,
        None,
        offline_mode,
        {
            let store_config = store_config.clone();
            let store_client = registry_client.clone();
            let store_paths = store_paths.clone();
            let store_tasks = store_tasks.clone();
            let store_task_limit = store_task_limit.clone();
            let progress_count = progress_count.clone();
            let progress_total = progress_total.clone();

            move |package| {
                let store_config = store_config.clone();
                let store_client = store_client.clone();
                let store_paths = store_paths.clone();
                let store_tasks = store_tasks.clone();
                let store_task_limit = store_task_limit.clone();
                let progress_count = progress_count.clone();
                let progress_total = progress_total.clone();
                let name = package.id.name.clone();

                async move {
                    let current = progress_count.fetch_add(1, Ordering::Relaxed) + 1;
                    let mut total = progress_total.load(Ordering::Relaxed);
                    if current > total {
                        total = current;
                        progress_total.store(total, Ordering::Relaxed);
                    }
                    console::progress("🚚", &name, current, total);

                    let package_id = package.id.clone();
                    let handle = tokio::spawn(async move {
                        let _permit =
                            store::acquire_store_task_permit(store_task_limit, &package_id).await?;
                        let path = store::ensure_package_with_offline(
                            &store_config,
                            &package,
                            &store_client,
                            offline_mode,
                        )
                        .await?;
                        let mut map = store_paths.lock().await;
                        map.insert(package_id, path);
                        Ok::<(), SnpmError>(())
                    });

                    let mut guard = store_tasks.lock().await;
                    guard.push(handle);
                    Ok(())
                }
            }
        },
    )
    .await?;

    let store_paths = collect_store_paths(&store_tasks, &store_paths).await?;
    Ok((graph, store_paths))
}

async fn collect_store_paths(
    store_tasks: &Arc<Mutex<Vec<JoinHandle<Result<()>>>>>,
    store_paths: &Arc<Mutex<BTreeMap<PackageId, PathBuf>>>,
) -> Result<BTreeMap<PackageId, PathBuf>> {
    let handles = {
        let mut guard = store_tasks.lock().await;
        std::mem::take(&mut *guard)
    };

    for handle in handles {
        let result = handle.await.map_err(|error| SnpmError::StoreTask {
            reason: error.to_string(),
        })?;
        result?;
    }

    let guard = store_paths.lock().await;
    Ok(guard.clone())
}
