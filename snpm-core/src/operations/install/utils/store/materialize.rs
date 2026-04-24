use crate::console;
use crate::resolve::{PackageId, ResolutionGraph, ResolvedPackage};
use crate::store;
use crate::{Result, SnpmConfig, SnpmError};

use futures::stream::{self, StreamExt};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub async fn materialize_missing_packages(
    config: &SnpmConfig,
    missing: &[ResolvedPackage],
    client: &reqwest::Client,
) -> Result<BTreeMap<PackageId, PathBuf>> {
    if missing.is_empty() {
        return Ok(BTreeMap::new());
    }

    materialize_packages(config, missing.iter().cloned(), client, Some("📦")).await
}

pub async fn materialize_store(
    config: &SnpmConfig,
    graph: &ResolutionGraph,
    client: &reqwest::Client,
) -> Result<BTreeMap<PackageId, PathBuf>> {
    materialize_packages(config, graph.packages.values().cloned(), client, None).await
}

async fn materialize_packages<I>(
    config: &SnpmConfig,
    packages: I,
    client: &reqwest::Client,
    progress_icon: Option<&'static str>,
) -> Result<BTreeMap<PackageId, PathBuf>>
where
    I: IntoIterator<Item = ResolvedPackage>,
{
    let packages: Vec<_> = packages.into_iter().collect();
    let concurrency = store::store_task_concurrency(config);
    let total = packages.len();
    let progress_count = Arc::new(AtomicUsize::new(0));
    let mut paths = BTreeMap::new();

    let mut results = stream::iter(packages.into_iter().map(|package| {
        let config = config.clone();
        let client = client.clone();
        let count = progress_count.clone();

        async move {
            if let Some(icon) = progress_icon {
                let current = count.fetch_add(1, Ordering::Relaxed) + 1;
                console::progress(icon, &package.id.name, current, total);
            }

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
