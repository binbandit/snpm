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
    /// Base URL of a remote side-effects cache (e.g.
    /// `https://cache.example.com/snpm`). When set, side-effects
    /// artifacts are first GET'd from `<url>/<cache-key>.tar.gz` on
    /// restore, and PUT back after a successful build. `None` disables
    /// the remote layer entirely (local cache still works).
    pub remote_cache_url: Option<String>,
    /// Bearer token sent as `Authorization: Bearer <token>` on remote
    /// cache requests. `None` means no auth header (suitable for
    /// open-bucket or in-cluster usage).
    pub remote_cache_auth_token: Option<String>,
    /// When `true`, snpm reads from the remote cache but never PUTs.
    /// Useful for CI pipelines that should consume but not pollute a
    /// shared cache, or for staging-against-prod scenarios.
    pub remote_cache_read_only: bool,
}

pub fn default_disable_global_virtual_store_for_packages() -> BTreeSet<String> {
    DEFAULT_DISABLE_GLOBAL_VIRTUAL_STORE_FOR_PACKAGES
        .into_iter()
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
impl SnpmConfig {
    /// A baseline config for unit tests. Nearly every test module used to
    /// hand-roll this full 24-field literal; construct the base here and
    /// override just the fields a given test cares about with struct
    /// update syntax: `SnpmConfig { data_dir, ..SnpmConfig::for_tests() }`.
    pub(crate) fn for_tests() -> Self {
        SnpmConfig {
            cache_dir: PathBuf::from("/tmp/cache"),
            data_dir: PathBuf::from("/tmp/data"),
            allow_scripts: BTreeSet::new(),
            disable_global_virtual_store_for_packages: BTreeSet::new(),
            min_package_age_days: None,
            min_package_cache_age_days: None,
            default_registry: "https://registry.npmjs.org".to_string(),
            scoped_registries: BTreeMap::new(),
            registry_auth: BTreeMap::new(),
            default_registry_auth_token: None,
            default_registry_auth_scheme: AuthScheme::Bearer,
            registry_auth_schemes: BTreeMap::new(),
            hoisting: HoistingMode::SingleVersion,
            link_backend: LinkBackend::Auto,
            strict_peers: false,
            frozen_lockfile_default: false,
            always_auth: false,
            registry_concurrency: 64,
            verbose: false,
            log_file: None,
            remote_cache_url: None,
            remote_cache_auth_token: None,
            remote_cache_read_only: false,
        }
    }
}
