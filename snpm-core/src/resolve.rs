use crate::registry::{RegistryPackage, RegistryVersion, fetch_package};
use crate::{Result, SnpmError};
use async_recursion::async_recursion;
use semver::{Version, VersionReq};
use std::collections::BTreeMap;

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

pub async fn resolve(root_deps: &BTreeMap<String, String>) -> Result<ResolutionGraph> {
    let mut packages = BTreeMap::new();
    let mut root_dependencies = BTreeMap::new();
    let mut package_cache = BTreeMap::new();

    for (name, range) in root_deps {
        let id = resolve_package(name, range, &mut packages, &mut package_cache).await?;
        let entry = RootDependency {
            requested: range.clone(),
            resolved: id,
        };

        root_dependencies.insert(name.clone(), entry);
    }

    let root = ResolutionRoot {
        dependencies: root_dependencies,
    };

    Ok(ResolutionGraph { root, packages })
}

#[async_recursion]
async fn resolve_package(
    name: &str,
    range: &str,
    packages: &mut BTreeMap<PackageId, ResolvedPackage>,
    package_cache: &mut BTreeMap<String, RegistryPackage>,
) -> Result<PackageId> {
    let package = if let Some(cached) = package_cache.get(name) {
        cached.clone()
    } else {
        let fetched = fetch_package(name).await?;
        package_cache.insert(name.to_string(), fetched.clone());
        fetched
    };

    let version_meta = select_version(name, range, &package)?;

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
        let dep_id = resolve_package(dep_name, dep_range, packages, package_cache).await?;
        dependencies.insert(dep_name.clone(), dep_id);
    }

    // Optional dependencies
    for (dep_name, dep_range) in version_meta.optional_dependencies.iter() {
        if let Ok(dep_id) = resolve_package(dep_name, dep_range, packages, package_cache).await {
            dependencies.insert(dep_name.clone(), dep_id);
        }
    }

    let resolved = ResolvedPackage {
        id: id.clone(),
        tarball: version_meta.dist.tarball.clone(),
        integrity: version_meta.dist.integrity.clone(),
        dependencies,
    };

    packages.insert(id.clone(), resolved);

    Ok(id)
}

fn select_version(name: &str, range: &str, package: &RegistryPackage) -> Result<RegistryVersion> {
    let normalized = if range == "latest" || range.is_empty() {
        "*"
    } else {
        range
    };

    let ranges = parse_range_set(name, range, normalized)?;

    let mut selected: Option<(Version, RegistryVersion)> = None;

    for (version_str, meta) in package.versions.iter() {
        let parsed = Version::parse(version_str);
        if let Ok(ver) = parsed {
            if matches_any_range(&ranges, &ver) {
                match &selected {
                    Some((best, _)) if ver <= *best => {}
                    _ => selected = Some((ver, meta.clone())),
                }
            }
        }
    }

    if let Some((_, meta)) = selected {
        Ok(meta)
    } else {
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
