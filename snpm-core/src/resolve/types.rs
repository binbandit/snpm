use crate::project::BinField;
use crate::registry::BundledDependencies;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct PackageId {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolvedPackage {
    pub id: PackageId,
    pub tarball: String,
    pub integrity: Option<String>,
    pub dependencies: BTreeMap<String, PackageId>,
    pub peer_dependencies: BTreeMap<String, String>,
    pub bundled_dependencies: Option<BundledDependencies>,
    pub has_bin: bool,
    pub bin: Option<BinField>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RootDependency {
    pub requested: String,
    pub resolved: PackageId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolutionRoot {
    pub dependencies: BTreeMap<String, RootDependency>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolutionGraph {
    pub root: ResolutionRoot,
    pub packages: BTreeMap<PackageId, ResolvedPackage>,
}
