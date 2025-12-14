use crate::registry::BundledDependencies;
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
    pub peer_dependencies: BTreeMap<String, String>,
    pub bundled_dependencies: Option<BundledDependencies>,
    pub has_bin: bool,
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
