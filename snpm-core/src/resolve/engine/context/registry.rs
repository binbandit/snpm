use super::{RegistryPrefetchRequest, ResolverContext, ResolverState};
use crate::config::OfflineMode;
use crate::registry::{RegistryPackage, RegistryProtocol};
use crate::{Result, SnpmConfig, console};

use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Mutex;

impl<'a> ResolverContext<'a> {
    pub(in crate::resolve) async fn fetch_registry_package(
        &self,
        cache_key: &str,
        source: &str,
        protocol: &RegistryProtocol,
    ) -> Result<Arc<RegistryPackage>> {
        fetch_registry_package_cached(
            &self.state,
            self.config,
            self.client,
            self.offline_mode,
            cache_key,
            source,
            protocol,
        )
        .await
    }

    pub(in crate::resolve) async fn schedule_registry_prefetch(
        &self,
        request: RegistryPrefetchRequest,
    ) {
        if !prefetchable_protocol(&request.protocol) {
            return;
        }

        if self
            .state
            .package_cache
            .read()
            .await
            .contains_key(&request.cache_key)
        {
            return;
        }

        let mut scheduled = self.state.prefetched_registry_keys.lock().await;
        if !scheduled.insert(request.cache_key.clone()) {
            return;
        }
        drop(scheduled);

        let _ = self.metadata_prefetch_tx.send(request);
    }
}

pub(crate) async fn prefetch_registry_request(
    state: ResolverState,
    config: SnpmConfig,
    client: Client,
    offline_mode: OfflineMode,
    request: RegistryPrefetchRequest,
) {
    if let Err(error) = fetch_registry_package_cached(
        &state,
        &config,
        &client,
        offline_mode,
        &request.cache_key,
        &request.source,
        &request.protocol,
    )
    .await
    {
        console::verbose(&format!(
            "packument prefetch miss for {} via {}: {}",
            request.source, request.protocol.name, error
        ));
    }
}

async fn fetch_registry_package_cached(
    state: &ResolverState,
    config: &SnpmConfig,
    client: &Client,
    offline_mode: OfflineMode,
    cache_key: &str,
    source: &str,
    protocol: &RegistryProtocol,
) -> Result<Arc<RegistryPackage>> {
    if let Some(package) = state.package_cache.read().await.get(cache_key).cloned() {
        return Ok(package);
    }

    let fetch_lock = package_fetch_lock(state, cache_key).await;
    let result = {
        let _fetch_guard = fetch_lock.lock().await;

        if let Some(package) = state.package_cache.read().await.get(cache_key).cloned() {
            Ok(package)
        } else {
            let _permit = state.registry_semaphore.acquire().await.map_err(|error| {
                crate::SnpmError::Internal {
                    reason: format!(
                        "registry semaphore closed while fetching {protocol:?} package {source}@{cache_key}: {error}"
                    ),
                }
            })?;

            if let Some(package) = state.package_cache.read().await.get(cache_key).cloned() {
                Ok(package)
            } else {
                let fetched = crate::registry::fetch_package_with_offline(
                    config,
                    client,
                    source,
                    protocol,
                    offline_mode,
                )
                .await?;

                let fetched = Arc::new(fetched);
                state
                    .package_cache
                    .write()
                    .await
                    .insert(cache_key.to_string(), fetched.clone());

                Ok(fetched)
            }
        }
    };

    cleanup_package_fetch_lock(state, cache_key, &fetch_lock).await;
    result
}

async fn package_fetch_lock(state: &ResolverState, cache_key: &str) -> Arc<Mutex<()>> {
    let mut locks = state.package_fetch_locks.lock().await;
    locks
        .entry(cache_key.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

async fn cleanup_package_fetch_lock(
    state: &ResolverState,
    cache_key: &str,
    fetch_lock: &Arc<Mutex<()>>,
) {
    let mut locks = state.package_fetch_locks.lock().await;
    let should_remove = locks
        .get(cache_key)
        .map(|existing| Arc::ptr_eq(existing, fetch_lock) && Arc::strong_count(existing) == 2)
        .unwrap_or(false);

    if should_remove {
        locks.remove(cache_key);
    }
}

fn prefetchable_protocol(protocol: &RegistryProtocol) -> bool {
    !matches!(protocol.name.as_str(), "file" | "git")
}
