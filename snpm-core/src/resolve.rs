use crate::registry::{RegistryPackage, RegistryProtocol, RegistryVersion, fetch_package};
use crate::{Result, SnpmConfig, SnpmError};
use async_recursion::async_recursion;
use semver::{Version, VersionReq};
use std::collections::BTreeMap;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

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

pub async fn resolve(
    config: &SnpmConfig,
    root_deps: &BTreeMap<String, String>,
    root_protocols: &BTreeMap<String, RegistryProtocol>,
    min_age_days: Option<u32>,
    force: bool,
    overrides: Option<&BTreeMap<String, String>>,
) -> Result<ResolutionGraph> {
    let mut packages = BTreeMap::new();
    let mut root_dependencies = BTreeMap::new();
    let mut package_cache = BTreeMap::new();
    let default_protocol = RegistryProtocol::npm();

    for (name, range) in root_deps {
        let protocol = root_protocols.get(name).unwrap_or(&default_protocol);

        let id = resolve_package(
            config,
            name,
            range,
            protocol,
            &mut packages,
            &mut package_cache,
            min_age_days,
            force,
            overrides,
        )
        .await?;
        let entry = RootDependency {
            requested: range.clone(),
            resolved: id,
        };

        root_dependencies.insert(name.clone(), entry);
    }

    let root = ResolutionRoot {
        dependencies: root_dependencies,
    };

    let graph = ResolutionGraph { root, packages };
    validate_peers(&graph)?;

    Ok(graph)
}

#[async_recursion]
async fn resolve_package(
    config: &SnpmConfig,
    name: &str,
    range: &str,
    protocol: &RegistryProtocol,
    packages: &mut BTreeMap<PackageId, ResolvedPackage>,
    package_cache: &mut BTreeMap<String, RegistryPackage>,
    min_age_days: Option<u32>,
    force: bool,
    overrides: Option<&BTreeMap<String, String>>,
) -> Result<PackageId> {
    let cache_key = format!("{:?}:{}", protocol, name);

    let package = if let Some(cached) = package_cache.get(&cache_key) {
        cached.clone()
    } else {
        let fetched = fetch_package(config, name, protocol).await?;
        package_cache.insert(cache_key.clone(), fetched.clone());
        fetched
    };

    let effective_range = if let Some(map) = overrides {
        map.get(name).map(|s| s.as_str()).unwrap_or(range)
    } else {
        range
    };

    let version_meta = select_version(name, effective_range, &package, min_age_days, force)?;

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

    if packages.contains_key(&id) {
        return Ok(id);
    }

    let mut dependencies = BTreeMap::new();

    // Regular dependencies
    for (dep_name, dep_range) in version_meta.dependencies.iter() {
        let dep_id = resolve_package(
            config,
            dep_name,
            dep_range,
            protocol,
            packages,
            package_cache,
            min_age_days,
            force,
            overrides,
        )
        .await?;
        dependencies.insert(dep_name.clone(), dep_id);
    }

    // Optional dependencies
    for (dep_name, dep_range) in version_meta.optional_dependencies.iter() {
        if let Ok(dep_id) = resolve_package(
            config,
            dep_name,
            dep_range,
            protocol,
            packages,
            package_cache,
            min_age_days,
            force,
            overrides,
        )
        .await
        {
            dependencies.insert(dep_name.clone(), dep_id);
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

    let resolved = ResolvedPackage {
        id: id.clone(),
        tarball: version_meta.dist.tarball.clone(),
        integrity: version_meta.dist.integrity.clone(),
        dependencies,
        peer_dependencies,
    };

    packages.insert(id.clone(), resolved);

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
            let normalized = if peer_range == "latest" || peer_range.is_empty() {
                "*"
            } else {
                peer_range.as_str()
            };

            let ranges = parse_range_set(peer_name, peer_range, normalized)?;

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
                if matches_any_range(&ranges, ver) {
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
    let normalized = if range == "latest" || range.is_empty() {
        "*"
    } else {
        range
    };

    let ranges = parse_range_set(name, range, normalized)?;
    let mut selected: Option<(Version, RegistryVersion)> = None;
    let now = OffsetDateTime::now_utc();
    let mut youngest_rejected: Option<(String, i64)> = None;

    for (version_str, meta) in package.versions.iter() {
        let parsed = Version::parse(version_str);
        if let Ok(ver) = parsed {
            if !matches_any_range(&ranges, &ver) {
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

fn parse_range_set(name: &str, original: &str, normalized: &str) -> Result<Vec<VersionReq>> {
    let parts: Vec<&str> = normalized
        .split("||")
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect();

    let mut ranges = Vec::new();

    for part in parts {
        let req = VersionReq::parse(part).map_err(|source| SnpmError::Semver {
            value: format!("{}@{}", name, original),
            source,
        })?;
        ranges.push(req);
    }

    if ranges.is_empty() {
        let req = VersionReq::parse("*").map_err(|source| SnpmError::Semver {
            value: format!("{}@{}", name, original),
            source,
        })?;
        ranges.push(req);
    }

    Ok(ranges)
}

fn matches_any_range(ranges: &[VersionReq], version: &Version) -> bool {
    for range in ranges {
        if range.matches(version) {
            return true;
        }
    }

    false
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
