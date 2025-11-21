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

    for (name, range) in root_deps {
        let id = resolve_package(name, range, &mut packages).await?;
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
) -> Result<PackageId> {
    let package = fetch_package(name).await?;
    let version_meta = select_version(name, range, &package)?;
    let id = PackageId {
        name: name.to_string(),
        version: version_meta.version.clone(),
    };

    if packages.contains_key(&id) {
        return Ok(id);
    }

    let mut dependencies = BTreeMap::new();

    for (dep_name, dep_range) in version_meta.dependencies.iter() {
        let dep_id = resolve_package(dep_name, dep_range, packages).await?;
        dependencies.insert(dep_name.clone(), dep_id);
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
    let normalized = if range == "latest" { "*" } else { range };
    let req = VersionReq::parse(normalized).map_err(|source| SnpmError::Semver {
        value: format!("{}@{}", name, range),
        source,
    })?;

    let mut selected: Option<(Version, RegistryVersion)> = None;

    for (version_str, meta) in package.versions.iter() {
        let parsed = Version::parse(version_str);
        if let Ok(ver) = parsed {
            if req.matches(&ver) {
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
