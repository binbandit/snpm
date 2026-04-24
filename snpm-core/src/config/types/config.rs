use super::{AuthScheme, HoistingMode, LinkBackend};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const DEFAULT_DISABLE_GLOBAL_VIRTUAL_STORE_FOR_PACKAGES: [&str; 5] =
    ["next", "nuxt", "vite", "vitepress", "parcel"];

#[derive(Debug, Clone)]
pub struct SnpmConfig {
    pub cache_dir: PathBuf,
    pub data_dir: PathBuf,
    pub allow_scripts: BTreeSet<String>,
    pub disable_global_virtual_store_for_packages: BTreeSet<String>,
    pub min_package_age_days: Option<u32>,
    pub min_package_cache_age_days: Option<u32>,
    pub default_registry: String,
    pub scoped_registries: BTreeMap<String, String>,
    pub registry_auth: BTreeMap<String, String>,
    pub default_registry_auth_token: Option<String>,
    pub default_registry_auth_scheme: AuthScheme,
    pub registry_auth_schemes: BTreeMap<String, AuthScheme>,
    pub hoisting: HoistingMode,
    pub link_backend: LinkBackend,
    pub strict_peers: bool,
    pub frozen_lockfile_default: bool,
    pub always_auth: bool,
    pub registry_concurrency: usize,
    pub verbose: bool,
    pub log_file: Option<PathBuf>,
}

pub fn default_disable_global_virtual_store_for_packages() -> BTreeSet<String> {
    DEFAULT_DISABLE_GLOBAL_VIRTUAL_STORE_FOR_PACKAGES
        .into_iter()
        .map(str::to_string)
        .collect()
}
