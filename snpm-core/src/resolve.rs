pub mod peers;
pub mod query;
pub mod types;

use crate::config::OfflineMode;
use crate::registry::{RegistryPackage, RegistryProtocol};
use crate::{Result, SnpmConfig, SnpmError, console};
use async_recursion::async_recursion;
use futures::future::{join, join_all};
use query::build_dep_request;
use reqwest::Client;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, Semaphore, mpsc};

pub use peers::validate_peers;
pub use types::*;

type PackageMap = BTreeMap<PackageId, ResolvedPackage>;
type PackageCache = BTreeMap<String, Arc<RegistryPackage>>;

/// Resolve dependencies with default online mode.
#[allow(clippy::too_many_arguments)]
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
    resolve_with_optional_roots(
        config,
        client,
        root_deps,
        root_protocols,
        &BTreeSet::new(),
        min_age_days,
        force,
        overrides,
        on_package,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn resolve_with_optional_roots<F, Fut>(
    config: &SnpmConfig,
    client: &Client,
    root_deps: &BTreeMap<String, String>,
    root_protocols: &BTreeMap<String, RegistryProtocol>,
    optional_root_names: &BTreeSet<String>,
    min_age_days: Option<u32>,
    force: bool,
    overrides: Option<&BTreeMap<String, String>>,
    on_package: F,
) -> Result<ResolutionGraph>
where
    F: FnMut(ResolvedPackage) -> Fut + Send,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    resolve_with_offline(
        config,
        client,
        root_deps,
        root_protocols,
        optional_root_names,
        min_age_days,
        force,
        overrides,
        OfflineMode::Online,
        on_package,
    )
    .await
}

/// Resolve dependencies respecting offline mode.
#[allow(clippy::too_many_arguments)]
pub async fn resolve_with_offline<F, Fut>(
    config: &SnpmConfig,
    client: &Client,
    root_deps: &BTreeMap<String, String>,
    root_protocols: &BTreeMap<String, RegistryProtocol>,
    optional_root_names: &BTreeSet<String>,
    min_age_days: Option<u32>,
    force: bool,
    overrides: Option<&BTreeMap<String, String>>,
    offline_mode: OfflineMode,
    on_package: F,
) -> Result<ResolutionGraph>
where
    F: FnMut(ResolvedPackage) -> Fut + Send,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    let packages = Arc::new(Mutex::new(PackageMap::new()));
    let package_cache = Arc::new(RwLock::new(PackageCache::new()));
    let default_protocol = RegistryProtocol::npm();
    let registry_semaphore = Arc::new(Semaphore::new(config.registry_concurrency));

    // Channel for pushing resolved packages to the prefetcher without polling
    let (prefetch_tx, prefetch_rx) = mpsc::unbounded_channel::<ResolvedPackage>();

    let resolver_task = async {
        let result = run_resolver(
            config,
            client,
            root_deps,
            root_protocols,
            optional_root_names,
            min_age_days,
            force,
            overrides,
            &default_protocol,
            packages.clone(),
            package_cache.clone(),
            registry_semaphore.clone(),
            offline_mode,
            prefetch_tx.clone(),
        )
        .await;

        // Drop sender so prefetcher sees channel closed
        drop(prefetch_tx);
        result
    };

    let prefetcher_task = run_prefetcher(prefetch_rx, on_package);

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

#[allow(clippy::too_many_arguments)]
async fn run_resolver(
    config: &SnpmConfig,
    client: &Client,
    root_deps: &BTreeMap<String, String>,
    root_protocols: &BTreeMap<String, RegistryProtocol>,
    optional_root_names: &BTreeSet<String>,
    min_age_days: Option<u32>,
    force: bool,
    overrides: Option<&BTreeMap<String, String>>,
    default_protocol: &RegistryProtocol,
    packages: Arc<Mutex<PackageMap>>,
    package_cache: Arc<RwLock<PackageCache>>,
    semaphore: Arc<Semaphore>,
    offline_mode: OfflineMode,
    prefetch_tx: mpsc::UnboundedSender<ResolvedPackage>,
) -> Result<ResolutionRoot> {
    let mut tasks = Vec::new();

    for (name, range) in root_deps {
        let protocol = root_protocols.get(name).unwrap_or(default_protocol).clone();
        let name = name.clone();
        let range = range.clone();
        let is_optional = optional_root_names.contains(&name);
        let packages = packages.clone();
        let package_cache = package_cache.clone();
        let semaphore = semaphore.clone();
        let prefetch_tx = prefetch_tx.clone();

        let task = async move {
            let result = resolve_package(
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
                semaphore,
                offline_mode,
                prefetch_tx,
            )
            .await;

            let id = match result {
                Ok(id) => id,
                Err(error) if is_optional => {
                    console::warn(&format!(
                        "Skipping optional dependency {}@{}: {}",
                        name, range, error
                    ));
                    return Ok::<Option<(String, RootDependency)>, SnpmError>(None);
                }
                Err(error) => return Err(error),
            };

            let root_dep = RootDependency {
                requested: range,
                resolved: id,
            };

            Ok::<Option<(String, RootDependency)>, SnpmError>(Some((name, root_dep)))
        };

        tasks.push(task);
    }

    let results = join_all(tasks).await;

    let mut root_dependencies = BTreeMap::new();

    for result in results {
        if let Some((name, dep)) = result? {
            root_dependencies.insert(name, dep);
        }
    }

    Ok(ResolutionRoot {
        dependencies: root_dependencies,
    })
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

    Ok::<(), SnpmError>(())
}

#[allow(clippy::too_many_arguments)]
#[async_recursion]
async fn resolve_package(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
    range: &str,
    protocol: &RegistryProtocol,
    packages: Arc<Mutex<PackageMap>>,
    package_cache: Arc<RwLock<PackageCache>>,
    min_age_days: Option<u32>,
    force: bool,
    overrides: Option<&BTreeMap<String, String>>,
    registry_semaphore: Arc<Semaphore>,
    offline_mode: OfflineMode,
    prefetch_tx: mpsc::UnboundedSender<ResolvedPackage>,
) -> Result<PackageId> {
    let request = build_dep_request(name, range, protocol, overrides);
    let cache_key = format!("{}:{}", request.protocol.name, request.source);

    let package = {
        // Fast path: read lock only
        let cached = {
            let cache = package_cache.read().await;
            cache.get(&cache_key).cloned()
        };

        if let Some(package) = cached {
            package
        } else {
            // Slow path: acquire semaphore, then double-check cache
            let _permit = registry_semaphore.acquire().await.unwrap();

            let rechecked = {
                let cache = package_cache.read().await;
                cache.get(&cache_key).cloned()
            };

            if let Some(package) = rechecked {
                drop(_permit);
                package
            } else {
                let fetched = crate::registry::fetch_package_with_offline(
                    config,
                    client,
                    &request.source,
                    &request.protocol,
                    offline_mode,
                )
                .await?;

                let fetched = Arc::new(fetched);
                {
                    let mut cache = package_cache.write().await;
                    cache.insert(cache_key, fetched.clone());
                }
                drop(_permit);
                fetched
            }
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

    // Fast path: skip placeholder construction if already resolved
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

    // Atomic check-and-insert: hold the lock across both to prevent duplicate
    // store tasks from being spawned for the same package
    {
        let mut packages_guard = packages.lock().await;
        if packages_guard.contains_key(&id) {
            return Ok(id);
        }
        packages_guard.insert(id.clone(), placeholder.clone());
    }

    // Notify prefetcher about the new package (ignore send errors if receiver dropped)
    let _ = prefetch_tx.send(placeholder.clone());

    let mut dependencies = BTreeMap::new();

    let mut dependency_futures = Vec::new();
    let mut optional_dependency_futures = Vec::new();

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
        let dep_protocol = protocol_from_range(&range);
        let semaphore = registry_semaphore.clone();
        let tx = prefetch_tx.clone();

        let future = async move {
            let id = resolve_package(
                config,
                client,
                &name,
                &range,
                &dep_protocol,
                packages_clone,
                cache_clone,
                min_age_days,
                force,
                overrides,
                semaphore,
                offline_mode,
                tx,
            )
            .await?;
            Ok::<(String, PackageId), SnpmError>((name, id))
        };

        dependency_futures.push(future);
    }

    for (dep_name, dep_range) in version_meta.optional_dependencies.iter() {
        if bundled_set.contains(dep_name) {
            continue;
        }

        let name = dep_name.clone();
        let range = dep_range.clone();
        let packages_clone = packages.clone();
        let cache_clone = package_cache.clone();
        let dep_protocol = protocol_from_range(&range);
        let semaphore = registry_semaphore.clone();
        let tx = prefetch_tx.clone();

        let future = async move {
            let result = resolve_package(
                config,
                client,
                &name,
                &range,
                &dep_protocol,
                packages_clone,
                cache_clone,
                min_age_days,
                force,
                overrides,
                semaphore,
                offline_mode,
                tx,
            )
            .await;

            match result {
                Ok(id) => Some((name, id)),
                Err(_) => None,
            }
        };

        optional_dependency_futures.push(future);
    }

    // Resolve regular and optional deps concurrently — the old code awaited
    // all regular deps before even starting optional deps, adding latency at
    // every level of the dependency tree.
    let (dep_results, optional_results) = join(
        join_all(dependency_futures),
        join_all(optional_dependency_futures),
    )
    .await;

    for result in dep_results {
        let (name, id) = result?;
        dependencies.insert(name, id);
    }

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

fn protocol_from_range(range: &str) -> RegistryProtocol {
    if range.starts_with("file:") {
        RegistryProtocol::file()
    } else if range.starts_with("git:") || range.starts_with("git+") {
        RegistryProtocol::git()
    } else if range.starts_with("jsr:") {
        RegistryProtocol::jsr()
    } else {
        RegistryProtocol::npm()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn resolve_relative_path_simple() {
        let base = Path::new("/packages/foo");
        let result = resolve_relative_path(base, "bar/baz");
        assert_eq!(result, PathBuf::from("/packages/foo/bar/baz"));
    }

    #[test]
    fn resolve_relative_path_with_parent() {
        let base = Path::new("/packages/foo");
        let result = resolve_relative_path(base, "../sibling");
        assert_eq!(result, PathBuf::from("/packages/sibling"));
    }

    #[test]
    fn resolve_relative_path_with_curdir() {
        let base = Path::new("/packages/foo");
        let result = resolve_relative_path(base, "./bar");
        assert_eq!(result, PathBuf::from("/packages/foo/bar"));
    }

    #[test]
    fn resolve_relative_path_root_resets() {
        let base = Path::new("/packages/foo");
        let result = resolve_relative_path(base, "/absolute/path");
        assert_eq!(result, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn resolve_relative_path_multiple_parent_dirs() {
        let base = Path::new("/a/b/c/d");
        let result = resolve_relative_path(base, "../../e");
        assert_eq!(result, PathBuf::from("/a/b/e"));
    }

    #[test]
    fn protocol_from_range_file() {
        assert_eq!(
            protocol_from_range("file:../local"),
            RegistryProtocol::file()
        );
    }

    #[test]
    fn protocol_from_range_git_colon() {
        assert_eq!(protocol_from_range("git:repo.git"), RegistryProtocol::git());
    }

    #[test]
    fn protocol_from_range_git_plus() {
        assert_eq!(
            protocol_from_range("git+https://github.com/foo/bar.git"),
            RegistryProtocol::git()
        );
    }

    #[test]
    fn protocol_from_range_jsr() {
        assert_eq!(
            protocol_from_range("jsr:@scope/pkg@^1.0.0"),
            RegistryProtocol::jsr()
        );
    }

    #[test]
    fn protocol_from_range_npm_default() {
        assert_eq!(protocol_from_range("^1.0.0"), RegistryProtocol::npm());
    }

    #[test]
    fn protocol_from_range_semver_range() {
        assert_eq!(
            protocol_from_range(">=2.0.0 <3.0.0"),
            RegistryProtocol::npm()
        );
    }
}
