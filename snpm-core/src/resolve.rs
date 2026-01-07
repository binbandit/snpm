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

    let resolver_task = run_resolver(
        config,
        client,
        root_deps,
        root_protocols,
        min_age_days,
        force,
        overrides,
        &default_protocol,
        packages.clone(),
        package_cache.clone(),
        done.clone(),
        registry_semaphore.clone(),
    );

    let prefetcher_task = run_prefetcher(packages.clone(), done.clone(), on_package);

    let (root_result, prefetch_result) = join(resolver_task, prefetcher_task).await;

    let root = root_result?;

    let mut packages_guard = packages.lock().await;
    let packages = std::mem::take(&mut *packages_guard);

    let graph = ResolutionGraph { root, packages };

    if let Err(error) = validate_peers(&graph) {
        if config.strict_peers {
            return Err(error);
        } else {
            console::warn(&format!(
                "peer dependency issues detected (non‑fatal): {error}"
            ));
        }
    }

    prefetch_result?;

    Ok(graph)
}

async fn run_resolver(
    config: &SnpmConfig,
    client: &Client,
    root_deps: &BTreeMap<String, String>,
    root_protocols: &BTreeMap<String, RegistryProtocol>,
    min_age_days: Option<u32>,
    force: bool,
    overrides: Option<&BTreeMap<String, String>>,
    default_protocol: &RegistryProtocol,
    packages: Arc<Mutex<PackageMap>>,
    package_cache: Arc<Mutex<PackageCache>>,
    done: Arc<AtomicBool>,
    semaphore: Arc<Semaphore>,
) -> Result<ResolutionRoot> {
    let mut tasks = Vec::new();

    for (name, range) in root_deps {
        let protocol = root_protocols.get(name).unwrap_or(default_protocol).clone();
        let name = name.clone();
        let range = range.clone();
        let packages = packages.clone();
        let package_cache = package_cache.clone();
        let semaphore = semaphore.clone();

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

    done.store(true, Ordering::SeqCst);

    Ok(ResolutionRoot {
        dependencies: root_dependencies,
    })
}

async fn run_prefetcher<F, Fut>(
    packages: Arc<Mutex<PackageMap>>,
    done: Arc<AtomicBool>,
    mut on_package: F,
) -> Result<()>
where
    F: FnMut(ResolvedPackage) -> Fut + Send,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    let mut seen = BTreeSet::new();

    loop {
        let snapshot: Vec<PackageId> = {
            let guard = packages.lock().await;
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
            if done.load(Ordering::SeqCst) {
                break;
            }

            yield_now().await;
            continue;
        }

        for id in new_ids {
            let package = {
                let guard = packages.lock().await;
                guard.get(&id).cloned()
            };

            if let Some(package) = package {
                on_package(package).await?;
            }
        }
    }

    Ok::<(), SnpmError>(())
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

        if let Some(package) = cached {
            package
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
        .map(|bundled_dependency| bundled_dependency.to_set(&version_meta.dependencies))
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

    if let Some(prefetch) = prefetch {
        prefetch.prefetch(&placeholder);
    }

    let mut dependencies = BTreeMap::new();

    let mut dependency_futures = Vec::new();

    for (dep_name, dep_range) in version_meta.dependencies.iter() {
        if bundled_set.contains(dep_name) {
            continue;
        }

        let name = dep_name.clone();

        let range =
            if version_meta.dist.tarball.starts_with("file://") && dep_range.starts_with("file:") {
                let base_pkg_path =
                    Path::new(version_meta.dist.tarball.strip_prefix("file://").unwrap());

                if let Some(relative_path) = dep_range.strip_prefix("file:") {
                    let resolved = resolve_relative_path(base_pkg_path, relative_path);
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

        let future = async move {
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

        dependency_futures.push(future);
    }

    let dep_results = join_all(dependency_futures).await;

    for result in dep_results {
        let (name, id) = result?;
        dependencies.insert(name, id);
    }

    let mut optional_dependency_futures = Vec::new();

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

        let future = async move {
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

        optional_dependency_futures.push(future);
    }

    let optional_results = join_all(optional_dependency_futures).await;

    for (name, id) in optional_results.into_iter().flatten() {
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

fn resolve_relative_path(base: &Path, relative: &str) -> PathBuf {
    let mut components = base.to_path_buf();

    for part in Path::new(relative).components() {
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
