use crate::registry::{RegistryPackage, RegistryProtocol, RegistryVersion, fetch_package};
use crate::{Result, SnpmConfig, SnpmError, console};
use async_recursion::async_recursion;
use futures::future::{join, join_all};
use futures::lock::Mutex;
use reqwest::Client;
use snpm_semver::{RangeSet, Version};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::Semaphore;
use tokio::task::yield_now;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PackageId {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug)]
pub struct ResolvedPackage {
    pub id: PackageId,
    pub tarball: String,
    pub integrity: Option<String>,
    pub dependencies: BTreeMap<String, PackageId>,
    pub peer_dependencies: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct RootDependency {
    pub requested: String,
    pub resolved: PackageId,
}

#[derive(Clone, Debug)]
pub struct ResolutionRoot {
    pub dependencies: BTreeMap<String, RootDependency>,
}

#[derive(Clone, Debug)]
pub struct ResolutionGraph {
    pub root: ResolutionRoot,
    pub packages: BTreeMap<PackageId, ResolvedPackage>,
}

pub trait Prefetcher: Send + Sync {
    fn prefetch(&self, package: &ResolvedPackage);
}

#[derive(Clone, Debug)]
struct DepRequest {
    source: String,
    range: String,
    protocol: RegistryProtocol,
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
            let fetched = fetch_package(config, client, &request.source, &request.protocol).await?;
            drop(_permit);

            let mut cache = package_cache.lock().await;
            cache.insert(cache_key, fetched.clone());
            fetched
        }
    };

    let version_meta = select_version(
        &request.source,
        &request.range,
        &package,
        min_age_days,
        force,
    )?;

    if !is_compatible(&version_meta.os, &version_meta.cpu) {
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

    let placeholder = ResolvedPackage {
        id: id.clone(),
        tarball: version_meta.dist.tarball.clone(),
        integrity: version_meta.dist.integrity.clone(),
        dependencies: BTreeMap::new(),
        peer_dependencies,
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
        let name = dep_name.clone();
        let range = dep_range.clone();
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

    for item in opt_results {
        if let Some((name, id)) = item {
            dependencies.insert(name, id);
        }
    }

    {
        let mut packages_guard = packages.lock().await;
        if let Some(existing) = packages_guard.get_mut(&id) {
            existing.dependencies = dependencies;
        }
    }

    Ok(id)
}

fn version_age_days(package: &RegistryPackage, version: &str, now: OffsetDateTime) -> Option<i64> {
    let time_str = package.time.get(version)?;
    let published = OffsetDateTime::parse(time_str, &Rfc3339).ok()?;
    let age = now - published;
    Some(age.whole_days())
}

fn validate_peers(graph: &ResolutionGraph) -> Result<()> {
    let mut versions_by_name = BTreeMap::new();

    for package in graph.packages.values() {
        if let Ok(ver) = Version::parse(&package.id.version) {
            versions_by_name
                .entry(package.id.name.clone())
                .or_insert_with(Vec::new)
                .push(ver);
        }
    }

    for package in graph.packages.values() {
        if package.peer_dependencies.is_empty() {
            continue;
        }

        for (peer_name, peer_range) in package.peer_dependencies.iter() {
            let range_set = parse_range_set(peer_name, peer_range)?;

            let candidates = match versions_by_name.get(peer_name) {
                Some(list) => list,
                None => {
                    return Err(SnpmError::ResolutionFailed {
                        name: package.id.name.clone(),
                        range: peer_range.clone(),
                        reason: format!("missing peer dependency {peer_name}"),
                    });
                }
            };

            let mut satisfied = false;

            for ver in candidates {
                if range_set.matches(ver) {
                    satisfied = true;
                    break;
                }
            }

            if !satisfied {
                let installed = candidates
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");

                return Err(SnpmError::ResolutionFailed {
                    name: package.id.name.clone(),
                    range: peer_range.clone(),
                    reason: format!(
                        "peer dependency {peer_name}@{peer_range} is not satisfied; installed versions: {installed}"
                    ),
                });
            }
        }
    }

    Ok(())
}

fn select_version(
    name: &str,
    range: &str,
    package: &RegistryPackage,
    min_age_days: Option<u32>,
    force: bool,
) -> Result<RegistryVersion> {
    let trimmed = range.trim();

    if trimmed == "latest" {
        if let Some(tag_version) = package.dist_tags.get("latest") {
            if let Some(meta) = package.versions.get(tag_version) {
                let now = OffsetDateTime::now_utc();

                if let Some(min_days) = min_age_days {
                    if !force {
                        if let Some(age_days) = version_age_days(package, &meta.version, now) {
                            if age_days < min_days as i64 {
                                return Err(SnpmError::ResolutionFailed {
                                    name: name.to_string(),
                                    range: range.to_string(),
                                    reason: format!(
                                        "latest dist-tag points to version {} which is only {} days old, less than the configured minimum of {} days",
                                        meta.version, age_days, min_days
                                    ),
                                });
                            }
                        }
                    }
                }

                return Ok(meta.clone());
            }
        }
    }

    let ranges = parse_range_set(name, range)?;
    let mut selected: Option<(Version, RegistryVersion)> = None;
    let now = OffsetDateTime::now_utc();
    let mut youngest_rejected: Option<(String, i64)> = None;

    for (version_str, meta) in package.versions.iter() {
        let parsed = Version::parse(version_str);
        if let Ok(ver) = parsed {
            if !ranges.matches(&ver) {
                continue;
            }

            if let Some(min_days) = min_age_days {
                if !force {
                    if let Some(age_days) = version_age_days(package, version_str, now) {
                        if age_days < min_days as i64 {
                            if youngest_rejected.is_none() {
                                youngest_rejected = Some((version_str.clone(), age_days));
                            }
                            continue;
                        }
                    }
                }
            }

            match &selected {
                Some((best, _)) if ver <= *best => {}
                _ => selected = Some((ver, meta.clone())),
            }
        }
    }

    if let Some((_, meta)) = selected {
        Ok(meta)
    } else {
        if let Some(min_days) = min_age_days {
            if !force {
                if let Some((ver_str, age_days)) = youngest_rejected {
                    return Err(SnpmError::ResolutionFailed {
                        name: name.to_string(),
                        range: range.to_string(),
                        reason: format!(
                            "latest matching version {ver_str} is only {age_days} days old, which is less than the configured minimum of {min_days} days"
                        ),
                    });
                }
            }
        }

        Err(SnpmError::ResolutionFailed {
            name: name.to_string(),
            range: range.to_string(),
            reason: "Version not found matching range".to_string(),
        })
    }
}

fn parse_range_set(name: &str, original: &str) -> Result<RangeSet> {
    RangeSet::parse(original).map_err(|err| SnpmError::Semver {
        value: format!("{}@{}", name, original),
        reason: err.to_string(),
    })
}

fn is_compatible(os: &[String], cpu: &[String]) -> bool {
    matches_os(os) && matches_cpu(cpu)
}

fn matches_os(list: &[String]) -> bool {
    if list.is_empty() {
        return true;
    }

    let current = current_os();
    let mut has_positive = false;
    let mut allowed = false;

    for entry in list {
        if let Some(negated) = entry.strip_prefix('!') {
            if negated == current {
                return false;
            }
        } else {
            has_positive = true;
            if entry == current {
                allowed = true;
            }
        }
    }

    if has_positive { allowed } else { true }
}

fn matches_cpu(list: &[String]) -> bool {
    if list.is_empty() {
        return true;
    }

    let current = current_cpu();
    let mut has_positive = false;
    let mut allowed = false;

    for entry in list {
        if let Some(negated) = entry.strip_prefix('!') {
            if negated == current {
                return false;
            }
        } else {
            has_positive = true;
            if entry == current {
                allowed = true;
            }
        }
    }

    if has_positive { allowed } else { true }
}

fn current_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

fn current_cpu() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "ia32",
        "aarch64" => "arm64",
        "arm" => "arm",
        other => other,
    }
}

fn split_protocol_spec(spec: &str) -> Option<(RegistryProtocol, String, String)> {
    let colon = spec.find(':')?;
    let (prefix, rest) = spec.split_at(colon);
    let rest = &rest[1..];

    let protocol = match prefix {
        "npm" => RegistryProtocol::npm(),
        "jsr" => RegistryProtocol::jsr(),
        other => RegistryProtocol::custom(other),
    };

    if rest.is_empty() {
        return None;
    }

    let mut source = rest.to_string();
    let mut range = "latest".to_string();

    if let Some(at) = rest.rfind('@') {
        let (name, ver_part) = rest.split_at(at);
        if !name.is_empty() {
            source = name.to_string();
        }
        let ver = ver_part.trim_start_matches('@');
        if !ver.is_empty() {
            range = ver.to_string()
        }
    }

    Some((protocol, source, range))
}

fn build_dep_request(
    name: &str,
    range: &str,
    protocol: &RegistryProtocol,
    overrides: Option<&BTreeMap<String, String>>,
) -> DepRequest {
    let overridden = overrides
        .and_then(|map| map.get(name))
        .map(|s| s.as_str())
        .unwrap_or(range);

    if let Some((proto, source, semver_range)) = split_protocol_spec(overridden) {
        DepRequest {
            source,
            range: semver_range,
            protocol: proto,
        }
    } else {
        DepRequest {
            source: name.to_string(),
            range: overridden.to_string(),
            protocol: protocol.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_matches_simple_range() {
        let ranges = parse_range_set("pkg", ">= 4.21.0").unwrap();
        let v = Version::parse("4.21.0").unwrap();
        assert!(ranges.matches(&v));
    }
}
