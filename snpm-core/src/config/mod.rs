use directories::ProjectDirs;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::PathBuf;

pub mod rc;
pub use self::rc::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthScheme {
    Bearer,
    Basic,
}

#[derive(Debug, Clone)]
pub struct SnpmConfig {
    pub cache_dir: PathBuf,
    pub data_dir: PathBuf,
    pub allow_scripts: BTreeSet<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkBackend {
    Auto,
    Hardlink,
    Symlink,
    Copy,
}

impl LinkBackend {
    pub fn from_str(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "auto" | "default" => Some(LinkBackend::Auto),
            "hardlink" | "hardlinks" | "hard" => Some(LinkBackend::Hardlink),
            "symlink" | "symlinks" | "symbolic" | "sym" => Some(LinkBackend::Symlink),
            "copy" | "copies" => Some(LinkBackend::Copy),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoistingMode {
    None,
    SingleVersion,
    All,
}

impl HoistingMode {
    pub fn from_str(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "none" | "off" | "false" | "disabled" => Some(HoistingMode::None),
            "single" | "single-version" | "safe" => Some(HoistingMode::SingleVersion),
            "root" | "all" | "true" | "on" | "enabled" => Some(HoistingMode::All),
            _ => None,
        }
    }
}

impl SnpmConfig {
    pub fn from_env() -> Self {
        let dirs = ProjectDirs::from("io", "snpm", "snpm");

        let (cache_dir, data_dir) = if let Ok(home) = env::var("SNPM_HOME") {
            let base = PathBuf::from(home);
            (base.join("cache"), base.join("data"))
        } else {
            match dirs {
                Some(dirs) => (
                    dirs.cache_dir().to_path_buf(),
                    dirs.data_local_dir().to_path_buf(),
                ),
                None => {
                    let fallback = PathBuf::from(".snpm");
                    (fallback.join("cache"), fallback.join("data"))
                }
            }
        };

        let allow_scripts = read_allow_scripts_from_env();
        let min_package_age_days = read_min_package_age_from_env();
        let min_package_cache_age_days = read_min_package_cache_age_from_env();

        let (
            runtime_config_default_registry,
            scoped_registries,
            registry_auth,
            runtime_config_default_auth_token,
            runtime_config_hoisting,
            runtime_config_default_auth_basic,
        ) = read_registry_config();

        let mut default_registry = runtime_config_default_registry;
        let mut default_registry_auth_token = runtime_config_default_auth_token;
        let mut hoisting = runtime_config_hoisting.unwrap_or(HoistingMode::SingleVersion);
        let mut link_backend = LinkBackend::Auto;
        let mut strict_peers = false;
        let mut frozen_lockfile_default = false;
        let mut registry_concurrency = 64;
        // Default to Bearer; may switch to Basic if _auth detected or token looks like Basic credentials
        let mut default_registry_auth_scheme = AuthScheme::Bearer;
        // Honor always-auth default (can be overridden by env below)
        let mut always_auth = false;

        if let Ok(value) =
            env::var("NPM_CONFIG_REGISTRY").or_else(|_| env::var("npm_config_registry"))
        {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                let new_default = trimmed.to_string();

                let runtime_config_host = host_from_url(&default_registry);
                let new_host = host_from_url(&new_default);

                if runtime_config_host != new_host {
                    default_registry_auth_token = None;
                }

                default_registry = normalize_registry_url(&new_default);
            }
        }

        if let Ok(token) = env::var("NODE_AUTH_TOKEN")
            .or_else(|_| env::var("NPM_TOKEN"))
            .or_else(|_| env::var("SNPM_AUTH_TOKEN"))
            .or_else(|_| env::var("NPM_CONFIG__AUTH"))
            .or_else(|_| env::var("npm_config__auth"))
        {
            let trimmed = token.trim();
            if !trimmed.is_empty() {
                default_registry_auth_token = Some(trimmed.to_string());
                // Heuristic: if token contains ':' or looks base64, prefer Basic to align with _auth semantics
                let looks_basic = trimmed.contains(':')
                    || trimmed
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "+/=_-.".contains(c));
                if looks_basic {
                    default_registry_auth_scheme = AuthScheme::Basic;
                }
            }
        }

        if let Ok(value) = env::var("SNPM_HOIST") {
            let trimmed = value.trim();
            if let Some(mode) = HoistingMode::from_str(trimmed) {
                hoisting = mode;
            }
        }

        if let Ok(value) = env::var("SNPM_LINK_BACKEND")
            && let Some(backend) = LinkBackend::from_str(value.trim())
        {
            link_backend = backend;
        }

        if let Ok(value) = env::var("SNPM_STRICT_PEERS") {
            let trimmed = value.trim().to_ascii_lowercase();
            strict_peers = matches!(trimmed.as_str(), "1" | "true" | "yes" | "y" | "on");
        }

        if let Ok(value) = env::var("SNPM_FROZEN_LOCKFILE") {
            let trimmed = value.trim().to_ascii_lowercase();
            frozen_lockfile_default = matches!(trimmed.as_str(), "1" | "true" | "yes" | "y" | "on");
        }

        if let Ok(value) = env::var("SNPM_REGISTRY_CONCURRENCY")
            && let Ok(parsed) = value.trim().parse::<usize>()
            && parsed > 0
        {
            registry_concurrency = parsed;
        }
        // Respect always-auth across env configs (pnpm/npm compatible names)
        if let Ok(value) = env::var("NPM_CONFIG_ALWAYS_AUTH")
            .or_else(|_| env::var("npm_config_always_auth"))
            .or_else(|_| env::var("SNPM_ALWAYS_AUTH"))
        {
            let on = match value.trim().to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "y" | "on" => true,
                _ => false,
            };
            if on {
                always_auth = true;
            }
        }

        let verbose = match env::var("SNPM_VERBOSE") {
            Ok(value) => {
                let v = value.trim().to_ascii_lowercase();
                matches!(v.as_str(), "1" | "true" | "yes" | "y" | "on")
            }
            Err(_) => false,
        };

        let log_file = env::var("SNPM_LOG_FILE")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .map(PathBuf::from);

        SnpmConfig {
            cache_dir,
            data_dir,
            allow_scripts,
            min_package_age_days,
            min_package_cache_age_days,
            default_registry,
            scoped_registries,
            registry_auth,
            default_registry_auth_token,
            // If runtime config parsing indicated _auth, prefer Basic for default registry
            default_registry_auth_scheme: if runtime_config_default_auth_basic {
                AuthScheme::Basic
            } else {
                default_registry_auth_scheme
            },
            registry_auth_schemes: BTreeMap::new(),
            hoisting,
            link_backend,
            strict_peers,
            frozen_lockfile_default,
            always_auth,
            registry_concurrency,
            verbose,
            log_file,
        }
    }

    pub fn packages_dir(&self) -> PathBuf {
        self.data_dir.join("packages")
    }

    pub fn metadata_dir(&self) -> PathBuf {
        self.data_dir.join("metadata")
    }

    pub fn auth_token_for_url(&self, url: &str) -> Option<&str> {
        let host = host_from_url(url)?;

        if let Some(token) = self.registry_auth.get(&host) {
            return Some(token.as_str());
        }

        if let Some(default_host) = host_from_url(&self.default_registry)
            && host == default_host
            && let Some(token) = self.default_registry_auth_token.as_ref()
        {
            return Some(token.as_str());
        }

        None
    }
}
