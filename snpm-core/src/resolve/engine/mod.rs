mod context;

use super::peers::validate_peers;
use super::types::{ResolutionGraph, ResolvedPackage};
use crate::config::OfflineMode;
use crate::{Result, SnpmConfig, console};
use futures::future::join;
use reqwest::Client;
use std::collections::{BTreeMap, BTreeSet};
use tokio::sync::mpsc;

pub(super) use context::ResolverContext;
use context::ResolverState;

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

    let resolver_context = ResolverContext {
        config,
        client,
        min_age_days,
        force,
        overrides,
        existing_graph,
        offline_mode,
        state: state.clone(),
        prefetch_tx: prefetch_tx.clone(),
    };

    let resolver_task = async move {
        let result = resolver_context
            .resolve_root_dependencies(root_deps, root_protocols, optional_root_names)
            .await;
        drop(prefetch_tx);
        result
    };

    let prefetcher_task = run_prefetcher(prefetch_rx, on_package);
    let (root_result, prefetch_result) = join(resolver_task, prefetcher_task).await;

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
