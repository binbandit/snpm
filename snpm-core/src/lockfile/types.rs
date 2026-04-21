use crate::project::BinField;
use crate::registry::BundledDependencies;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(super) const LOCKFILE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockRootDependency {
    pub requested: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockRoot {
    pub dependencies: BTreeMap<String, LockRootDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockPackage {
    pub name: String,
    pub version: String,
    pub tarball: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integrity: Option<String>,
    pub dependencies: BTreeMap<String, String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "bundledDependencies"
    )]
    pub bundled_dependencies: Option<BundledDependencies>,
    #[serde(default, skip_serializing_if = "is_false", rename = "hasBin")]
    pub has_bin: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<BinField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lockfile {
    pub version: u32,
    pub root: LockRoot,
    pub packages: BTreeMap<String, LockPackage>,
}

fn is_false(value: &bool) -> bool {
    !*value
}
