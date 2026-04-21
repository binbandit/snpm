mod context;

use super::peers::validate_peers;
use super::types::{ResolutionGraph, ResolvedPackage};
use crate::config::OfflineMode;
use crate::{Result, SnpmConfig, console};
use futures::future::join3;
use reqwest::Client;
use std::collections::{BTreeMap, BTreeSet};
use tokio::sync::mpsc;
use tokio::task::JoinSet;

#[cfg(test)]
pub(crate) use context::ResolverState;
pub(super) use context::{RegistryPrefetchRequest, ResolverContext};

#[cfg(not(test))]
use context::ResolverState;
use context::prefetch_registry_request;

#[allow(clippy::too_many_arguments)]
pub async fn resolve_with_offline<F, Fut>(
    config: &SnpmConfig,
    client: &Client,
    root_deps: &BTreeMap<String, String>,
    root_protocols: &BTreeMap<String, crate::registry::RegistryProtocol>,
    optional_root_names: &BTreeSet<String>,
    min_age_days: Option<u32>,
    force: bool,
    overrides: Option<&BTreeMap<String, String>>,
    workspace_sources: Option<&BTreeMap<String, String>>,
    existing_graph: Option<&super::types::ResolutionGraph>,
    offline_mode: OfflineMode,
    on_package: F,
) -> Result<ResolutionGraph>
where
    F: FnMut(ResolvedPackage) -> Fut + Send,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    let state = ResolverState::new(config.registry_concurrency);
    let (prefetch_tx, prefetch_rx) = mpsc::unbounded_channel();
    let (metadata_prefetch_tx, metadata_prefetch_rx) = mpsc::unbounded_channel();

    let resolver_context = ResolverContext {
        config,
        client,
        min_age_days,
        force,
        overrides,
        workspace_sources,
        existing_graph,
        offline_mode,
        state: state.clone(),
        prefetch_tx: prefetch_tx.clone(),
        metadata_prefetch_tx: metadata_prefetch_tx.clone(),
    };

    let resolver_task = async move {
        let result = resolver_context
            .resolve_root_dependencies(root_deps, root_protocols, optional_root_names)
            .await;
        drop(prefetch_tx);
        drop(metadata_prefetch_tx);
        result
    };

    let prefetcher_task = run_prefetcher(prefetch_rx, on_package);
    let metadata_prefetcher_task = run_metadata_prefetcher(
        config.clone(),
        client.clone(),
        state.clone(),
        offline_mode,
        metadata_prefetch_rx,
    );
    let (root_result, prefetch_result, metadata_prefetch_result) =
        join3(resolver_task, prefetcher_task, metadata_prefetcher_task).await;

    let graph = ResolutionGraph {
        root: root_result?,
        packages: state.take_packages().await,
    };

    if let Err(error) = validate_peers(&graph) {
        if config.strict_peers {
            return Err(error);
        }

        console::warn(&format!(
            "peer dependency issues detected (non‑fatal): {error}"
        ));
    }

    prefetch_result?;
    metadata_prefetch_result?;

    Ok(graph)
}

async fn run_prefetcher<F, Fut>(
    mut rx: mpsc::UnboundedReceiver<ResolvedPackage>,
    mut on_package: F,
) -> Result<()>
where
    F: FnMut(ResolvedPackage) -> Fut + Send,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    while let Some(package) = rx.recv().await {
        on_package(package).await?;
    }

    Ok(())
}

async fn run_metadata_prefetcher(
    config: SnpmConfig,
    client: Client,
    state: ResolverState,
    offline_mode: OfflineMode,
    mut rx: mpsc::UnboundedReceiver<RegistryPrefetchRequest>,
) -> Result<()> {
    let concurrency = config.registry_concurrency.max(1);
    let mut tasks = JoinSet::new();
    let mut open = true;

    loop {
        while open && tasks.len() < concurrency {
            match rx.recv().await {
                Some(request) => {
                    let config = config.clone();
                    let client = client.clone();
                    let state = state.clone();
                    tasks.spawn(async move {
                        prefetch_registry_request(state, config, client, offline_mode, request)
                            .await;
                    });
                }
                None => {
                    open = false;
                }
            }
        }

        if !open && tasks.is_empty() {
            break;
        }

        if let Some(result) = tasks.join_next().await
            && let Err(error) = result
        {
            console::verbose(&format!("packument prefetch task failed: {error}"));
        }
    }

    Ok(())
}
