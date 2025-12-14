pub mod peers;
pub mod query;
pub mod types;

use crate::registry::{RegistryPackage, RegistryProtocol};
use crate::{Result, SnpmConfig, SnpmError, console};
use async_recursion::async_recursion;
use futures::future::{join, join_all};
use futures::lock::Mutex;
use query::build_dep_request;
use reqwest::Client;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Semaphore;
use tokio::task::yield_now;

pub use peers::validate_peers;
pub use types::*;

pub trait Prefetcher: Send + Sync {
    fn prefetch(&self, package: &ResolvedPackage);
}

type PackageMap = BTreeMap<PackageId, ResolvedPackage>;
type PackageCache = BTreeMap<String, RegistryPackage>;

pub async fn resolve<F, Fut>(
    config: &SnpmConfig,
    client: &Client,
    root_deps: &BTreeMap<String, String>,
    root_protocols: &BTreeMap<String, RegistryProtocol>,
    min_age_days: Option<u32>,
    force: bool,
    overrides: Option<&BTreeMap<String, String>>,
    on_package: F,
) -> Result<ResolutionGraph>
where
    F: FnMut(ResolvedPackage) -> Fut + Send,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    let packages = Arc::new(Mutex::new(PackageMap::new()));
    let package_cache = Arc::new(Mutex::new(PackageCache::new()));
    let default_protocol = RegistryProtocol::npm();
    let done = Arc::new(AtomicBool::new(false));
    let registry_semaphore = Arc::new(Semaphore::new(config.registry_concurrency));

    let packages_for_resolver = packages.clone();
    let package_cache_for_resolver = package_cache.clone();
    let done_for_resolver = done.clone();
    let semaphore_for_resolver = registry_semaphore.clone();

    let resolver = async move {
        let result: Result<ResolutionRoot> = async {
            let mut tasks = Vec::new();

            for (name, range) in root_deps {
                let protocol = root_protocols
                    .get(name)
                    .unwrap_or(&default_protocol)
                    .clone();
                let name = name.clone();
                let range = range.clone();
                let packages = packages_for_resolver.clone();
                let package_cache = package_cache_for_resolver.clone();
                let semaphore = semaphore_for_resolver.clone();

                let task = async move {
                    let id = resolve_package(
                        config,
                        client,
                        &name,
                        &range,
                        &protocol,
                        packages,
                        package_cache,
                        min_age_days,
                        force,
                        overrides,
                        None,
                        semaphore,
                    )
                    .await?;

                    let root_dep = RootDependency {
                        requested: range,
                        resolved: id,
                    };

                    Ok::<(String, RootDependency), SnpmError>((name, root_dep))
                };

                tasks.push(task);
            }

            let results = join_all(tasks).await;

            let mut root_dependencies = BTreeMap::new();

            for result in results {
                let (name, dep) = result?;
                root_dependencies.insert(name, dep);
            }

            let root = ResolutionRoot {
                dependencies: root_dependencies,
            };

            Ok(root)
        }
        .await;

        done_for_resolver.store(true, Ordering::SeqCst);

        result
    };

    let packages_for_prefetch = packages.clone();
    let done_for_prefetch = done.clone();
    let callback = on_package;

    let prefetcher = async move {
        let mut seen = BTreeSet::new();
        let mut on_package = callback;

        loop {
            let snapshot: Vec<PackageId> = {
                let guard = packages_for_prefetch.lock().await;
                guard.keys().cloned().collect()
            };

            let mut new_ids = Vec::new();

            for id in snapshot {
                if !seen.contains(&id) {
                    seen.insert(id.clone());
                    new_ids.push(id);
                }
            }

            if new_ids.is_empty() {
                if done_for_prefetch.load(Ordering::SeqCst) {
                    break;
                }

                yield_now().await;
                continue;
            }

            for id in new_ids {
                let pkg = {
                    let guard = packages_for_prefetch.lock().await;
                    guard.get(&id).cloned()
                };

                if let Some(pkg) = pkg {
                    on_package(pkg).await?;
                }
            }
        }

        Ok::<(), SnpmError>(())
    };

    let (root_result, prefetch_result) = join(resolver, prefetcher).await;

    let root = root_result?;

    let mut packages_guard = packages.lock().await;
    let packages = std::mem::take(&mut *packages_guard);

    let graph = ResolutionGraph { root, packages };

    if let Err(err) = validate_peers(&graph) {
        if config.strict_peers {
            return Err(err);
        } else {
            console::warn(&format!(
                "peer dependency issues detected (non‑fatal): {err}"
            ));
        }
    }

    prefetch_result?;

    Ok(graph)
}

#[async_recursion]
async fn resolve_package(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
    range: &str,
    protocol: &RegistryProtocol,
    packages: Arc<Mutex<PackageMap>>,
    package_cache: Arc<Mutex<PackageCache>>,
    min_age_days: Option<u32>,
    force: bool,
    overrides: Option<&BTreeMap<String, String>>,
    prefetch: Option<&dyn Prefetcher>,
    registry_semaphore: Arc<Semaphore>,
) -> Result<PackageId> {
    let request = build_dep_request(name, range, protocol, overrides);
    let cache_key = format!("{:?}:{}", request.protocol, request.source);

    let package = {
        let cached = {
            let cache = package_cache.lock().await;
            cache.get(&cache_key).cloned()
        };

        if let Some(pkg) = cached {
            pkg
        } else {
            let _permit = registry_semaphore.acquire().await.unwrap();
            let fetched =
                crate::registry::fetch_package(config, client, &request.source, &request.protocol)
                    .await?;
            drop(_permit);

            let mut cache = package_cache.lock().await;
            cache.insert(cache_key, fetched.clone());
            fetched
        }
    };

    let version_meta = crate::version::select_version(
        &request.source,
        &request.range,
        &package,
        min_age_days,
        force,
    )?;

    if !crate::platform::is_compatible(&version_meta.os, &version_meta.cpu) {
        return Err(SnpmError::ResolutionFailed {
            name: name.to_string(),
            range: range.to_string(),
            reason: "package is not compatible with current OS/CPU".to_string(),
        });
    }

    let id = PackageId {
        name: name.to_string(),
        version: version_meta.version.clone(),
    };

    {
        let packages_guard = packages.lock().await;
        if packages_guard.contains_key(&id) {
            return Ok(id);
        }
    }

    let mut peer_dependencies = BTreeMap::new();

    for (peer_name, peer_range) in version_meta.peer_dependencies.iter() {
        let is_optional = version_meta
            .peer_dependencies_meta
            .get(peer_name)
            .map(|m| m.optional)
            .unwrap_or(false);

        if !is_optional {
            peer_dependencies.insert(peer_name.clone(), peer_range.clone());
        }
    }

    let bundled_deps = version_meta.get_bundled_dependencies().cloned();
    let bundled_set = bundled_deps
        .as_ref()
        .map(|bd| bd.to_set(&version_meta.dependencies))
        .unwrap_or_default();

    let placeholder = ResolvedPackage {
        id: id.clone(),
        tarball: version_meta.dist.tarball.clone(),
        integrity: version_meta.dist.integrity.clone(),
        dependencies: BTreeMap::new(),
        peer_dependencies,
        bundled_dependencies: bundled_deps.clone(),
        has_bin: version_meta.has_bin(),
    };

    {
        let mut packages_guard = packages.lock().await;
        packages_guard.insert(id.clone(), placeholder.clone());
    }

    if let Some(p) = prefetch {
        p.prefetch(&placeholder);
    }

    let mut dependencies = BTreeMap::new();

    let mut dep_futures = Vec::new();

    for (dep_name, dep_range) in version_meta.dependencies.iter() {
        if bundled_set.contains(dep_name) {
            continue;
        }

        let name = dep_name.clone();

        let range =
            if version_meta.dist.tarball.starts_with("file://") && dep_range.starts_with("file:") {
                let base_pkg_path =
                    Path::new(version_meta.dist.tarball.strip_prefix("file://").unwrap());

                if let Some(rel_path) = dep_range.strip_prefix("file:") {
                    let resolved = resolve_relative_path(base_pkg_path, rel_path);
                    format!("file:{}", resolved.display())
                } else {
                    dep_range.clone()
                }
            } else {
                dep_range.clone()
            };

        let packages_clone = packages.clone();
        let cache_clone = package_cache.clone();
        let protocol = request.protocol.clone();
        let prefetch = prefetch;
        let semaphore = registry_semaphore.clone();

        let fut = async move {
            let id = resolve_package(
                config,
                client,
                &name,
                &range,
                &protocol,
                packages_clone,
                cache_clone,
                min_age_days,
                force,
                overrides,
                prefetch,
                semaphore,
            )
            .await?;
            Ok::<(String, PackageId), SnpmError>((name, id))
        };

        dep_futures.push(fut);
    }

    let dep_results = join_all(dep_futures).await;

    for result in dep_results {
        let (name, id) = result?;
        dependencies.insert(name, id);
    }

    let mut opt_futures = Vec::new();

    for (dep_name, dep_range) in version_meta.optional_dependencies.iter() {
        if bundled_set.contains(dep_name) {
            continue;
        }

        let name = dep_name.clone();
        let range = dep_range.clone();
        let packages_clone = packages.clone();
        let cache_clone = package_cache.clone();
        let protocol = request.protocol.clone();
        let prefetch = prefetch;
        let semaphore = registry_semaphore.clone();

        let fut = async move {
            let result = resolve_package(
                config,
                client,
                &name,
                &range,
                &protocol,
                packages_clone,
                cache_clone,
                min_age_days,
                force,
                overrides,
                prefetch,
                semaphore,
            )
            .await;

            match result {
                Ok(id) => Some((name, id)),
                Err(_) => None,
            }
        };

        opt_futures.push(fut);
    }

    let opt_results = join_all(opt_futures).await;

    for (name, id) in opt_results.into_iter().flatten() {
        dependencies.insert(name, id);
    }

    {
        let mut packages_guard = packages.lock().await;
        if let Some(existing) = packages_guard.get_mut(&id) {
            existing.dependencies = dependencies;
        }
    }

    Ok(id)
}

fn resolve_relative_path(base: &Path, rel: &str) -> PathBuf {
    let mut components = base.to_path_buf();

    for part in Path::new(rel).components() {
        use std::path::Component;
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                components.pop();
            }
            Component::Normal(c) => components.push(c),
            Component::RootDir => {
                components = PathBuf::from("/");
            }
            Component::Prefix(_) => {}
        }
    }

    components
}
