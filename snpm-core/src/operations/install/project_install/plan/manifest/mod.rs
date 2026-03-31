mod additions;
mod resolve;
mod root;

use crate::registry::RegistryProtocol;

use std::collections::{BTreeMap, BTreeSet};

pub(super) use additions::collect_additions;
pub(super) use resolve::resolve_manifest_specs;
pub(super) use root::{build_root_protocols, build_root_specs, merge_root_dependencies};

pub(crate) struct ResolvedManifestSpecs {
    pub(crate) local_deps: BTreeSet<String>,
    pub(crate) local_dev_deps: BTreeSet<String>,
    pub(crate) local_optional_deps: BTreeSet<String>,
    pub(crate) dependencies: BTreeMap<String, String>,
    pub(crate) development_dependencies: BTreeMap<String, String>,
    pub(crate) optional_dependencies: BTreeMap<String, String>,
    pub(crate) protocols: BTreeMap<String, RegistryProtocol>,
}
