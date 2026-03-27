use directories::ProjectDirs;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::PathBuf;

pub mod rc;
pub use self::rc::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OfflineMode {
    /// Normal mode: fetch from network, use cache as optimization
    #[default]
    Online,
    /// Prefer cached data even if stale; only fetch if cache miss
    PreferOffline,
    /// Never fetch from network; fail if not cached
    Offline,
}

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
    Reflink,
    Hardlink,
    Symlink,
    Copy,
}

impl LinkBackend {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "auto" | "default" => Some(LinkBackend::Auto),
            "reflink" | "reflinks" | "cow" | "clone" => Some(LinkBackend::Reflink),
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
    pub fn parse(value: &str) -> Option<Self> {
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

        let runtime_config = read_registry_config();
        let runtime_config_default_auth_basic = runtime_config.default_auth_basic;

        let mut default_registry = runtime_config.default_registry;
        let scoped_registries = runtime_config.scoped;
        let registry_auth = runtime_config.registry_auth;
        let registry_auth_schemes = runtime_config.registry_auth_schemes;
        let mut default_registry_auth_token = runtime_config.default_auth_token;
        let mut hoisting = runtime_config
            .hoisting
            .unwrap_or(HoistingMode::SingleVersion);
        let mut link_backend = LinkBackend::Auto;
        let mut strict_peers = false;
        let mut frozen_lockfile_default = false;
        let mut registry_concurrency = 128;
        // Default to Bearer; may switch to Basic if _auth detected or token looks like Basic credentials
        let mut default_registry_auth_scheme = AuthScheme::Bearer;
        // Honor always-auth from rc files (can be overridden by env below)
        let mut always_auth = runtime_config.always_auth;

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

        // Bearer token env vars — these are always used as Bearer auth
        if let Ok(token) = env::var("NODE_AUTH_TOKEN")
            .or_else(|_| env::var("NPM_TOKEN"))
            .or_else(|_| env::var("SNPM_AUTH_TOKEN"))
        {
            let trimmed = token.trim();
            if !trimmed.is_empty() {
                default_registry_auth_token = Some(trimmed.to_string());
                default_registry_auth_scheme = AuthScheme::Bearer;
            }
        }
        // _auth env vars — these contain base64-encoded credentials for Basic auth
        else if let Ok(token) =
            env::var("NPM_CONFIG__AUTH").or_else(|_| env::var("npm_config__auth"))
        {
            let trimmed = token.trim();
            if !trimmed.is_empty() {
                default_registry_auth_token = Some(trimmed.to_string());
                default_registry_auth_scheme = AuthScheme::Basic;
            }
        }

        if let Ok(value) = env::var("SNPM_HOIST") {
            let trimmed = value.trim();
            if let Some(mode) = HoistingMode::parse(trimmed) {
                hoisting = mode;
            }
        }

        if let Ok(value) = env::var("SNPM_LINK_BACKEND")
            && let Some(backend) = LinkBackend::parse(value.trim())
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
            let on = matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "y" | "on"
            );
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
            registry_auth_schemes,
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

    pub fn global_dir(&self) -> PathBuf {
        self.data_dir.join("global")
    }

    pub fn global_bin_dir(&self) -> PathBuf {
        self.data_dir.join("bin")
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

    pub fn auth_scheme_for_url(&self, url: &str) -> AuthScheme {
        if let Some(host) = host_from_url(url)
            && let Some(scheme) = self.registry_auth_schemes.get(&host)
        {
            return *scheme;
        }
        self.default_registry_auth_scheme
    }

    pub fn authorization_header_for_url(&self, url: &str) -> Option<String> {
        let token = self.auth_token_for_url(url)?;
        let scheme = self.auth_scheme_for_url(url);

        Some(match scheme {
            AuthScheme::Bearer => format!("Bearer {}", token),
            AuthScheme::Basic => format!("Basic {}", token),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    #[test]
    fn authorization_header_uses_scoped_basic_auth() {
        let mut registry_auth = BTreeMap::new();
        registry_auth.insert(
            "registry.example.com".to_string(),
            "dXNlcjpwYXNz".to_string(),
        );

        let mut registry_auth_schemes = BTreeMap::new();
        registry_auth_schemes.insert("registry.example.com".to_string(), AuthScheme::Basic);

        let config = SnpmConfig {
            cache_dir: PathBuf::from("/tmp/cache"),
            data_dir: PathBuf::from("/tmp/data"),
            allow_scripts: BTreeSet::new(),
            min_package_age_days: None,
            min_package_cache_age_days: None,
            default_registry: "https://registry.npmjs.org/".to_string(),
            scoped_registries: BTreeMap::new(),
            registry_auth,
            default_registry_auth_token: None,
            default_registry_auth_scheme: AuthScheme::Bearer,
            registry_auth_schemes,
            hoisting: HoistingMode::SingleVersion,
            link_backend: LinkBackend::Auto,
            strict_peers: false,
            frozen_lockfile_default: false,
            always_auth: false,
            registry_concurrency: 64,
            verbose: false,
            log_file: None,
        };

        let header = config
            .authorization_header_for_url("https://registry.example.com/pkg.tgz")
            .expect("header");

        assert_eq!(header, "Basic dXNlcjpwYXNz");
    }

    fn make_config() -> SnpmConfig {
        SnpmConfig {
            cache_dir: PathBuf::from("/tmp/cache"),
            data_dir: PathBuf::from("/tmp/data"),
            allow_scripts: BTreeSet::new(),
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
        }
    }

    #[test]
    fn link_backend_parse_auto() {
        assert_eq!(LinkBackend::parse("auto"), Some(LinkBackend::Auto));
        assert_eq!(LinkBackend::parse("default"), Some(LinkBackend::Auto));
    }

    #[test]
    fn link_backend_parse_reflink() {
        assert_eq!(LinkBackend::parse("reflink"), Some(LinkBackend::Reflink));
        assert_eq!(LinkBackend::parse("cow"), Some(LinkBackend::Reflink));
        assert_eq!(LinkBackend::parse("clone"), Some(LinkBackend::Reflink));
    }

    #[test]
    fn link_backend_parse_hardlink() {
        assert_eq!(LinkBackend::parse("hardlink"), Some(LinkBackend::Hardlink));
        assert_eq!(LinkBackend::parse("hard"), Some(LinkBackend::Hardlink));
    }

    #[test]
    fn link_backend_parse_symlink() {
        assert_eq!(LinkBackend::parse("symlink"), Some(LinkBackend::Symlink));
        assert_eq!(LinkBackend::parse("sym"), Some(LinkBackend::Symlink));
    }

    #[test]
    fn link_backend_parse_copy() {
        assert_eq!(LinkBackend::parse("copy"), Some(LinkBackend::Copy));
        assert_eq!(LinkBackend::parse("copies"), Some(LinkBackend::Copy));
    }

    #[test]
    fn link_backend_parse_unknown() {
        assert_eq!(LinkBackend::parse("unknown"), None);
    }

    #[test]
    fn link_backend_parse_case_insensitive() {
        assert_eq!(LinkBackend::parse("AUTO"), Some(LinkBackend::Auto));
        assert_eq!(LinkBackend::parse("Hardlink"), Some(LinkBackend::Hardlink));
    }

    #[test]
    fn hoisting_mode_parse_none() {
        assert_eq!(HoistingMode::parse("none"), Some(HoistingMode::None));
        assert_eq!(HoistingMode::parse("off"), Some(HoistingMode::None));
        assert_eq!(HoistingMode::parse("false"), Some(HoistingMode::None));
        assert_eq!(HoistingMode::parse("disabled"), Some(HoistingMode::None));
    }

    #[test]
    fn hoisting_mode_parse_single() {
        assert_eq!(
            HoistingMode::parse("single"),
            Some(HoistingMode::SingleVersion)
        );
        assert_eq!(
            HoistingMode::parse("single-version"),
            Some(HoistingMode::SingleVersion)
        );
        assert_eq!(
            HoistingMode::parse("safe"),
            Some(HoistingMode::SingleVersion)
        );
    }

    #[test]
    fn hoisting_mode_parse_all() {
        assert_eq!(HoistingMode::parse("root"), Some(HoistingMode::All));
        assert_eq!(HoistingMode::parse("all"), Some(HoistingMode::All));
        assert_eq!(HoistingMode::parse("true"), Some(HoistingMode::All));
    }

    #[test]
    fn hoisting_mode_parse_unknown() {
        assert_eq!(HoistingMode::parse("unknown"), None);
    }

    #[test]
    fn auth_token_for_url_returns_scoped_token() {
        let mut config = make_config();
        config
            .registry_auth
            .insert("custom.registry.com".to_string(), "my-token".to_string());

        assert_eq!(
            config.auth_token_for_url("https://custom.registry.com/pkg.tgz"),
            Some("my-token")
        );
    }

    #[test]
    fn auth_token_for_url_returns_default_token_for_default_registry() {
        let mut config = make_config();
        config.default_registry_auth_token = Some("default-token".to_string());

        assert_eq!(
            config.auth_token_for_url("https://registry.npmjs.org/pkg.tgz"),
            Some("default-token")
        );
    }

    #[test]
    fn auth_token_for_url_returns_none_for_unknown() {
        let config = make_config();
        assert_eq!(
            config.auth_token_for_url("https://unknown.registry.com/pkg.tgz"),
            None
        );
    }

    #[test]
    fn auth_scheme_for_url_returns_scoped_scheme() {
        let mut config = make_config();
        config
            .registry_auth_schemes
            .insert("custom.registry.com".to_string(), AuthScheme::Basic);

        assert_eq!(
            config.auth_scheme_for_url("https://custom.registry.com/pkg"),
            AuthScheme::Basic
        );
    }

    #[test]
    fn auth_scheme_for_url_returns_default() {
        let config = make_config();
        assert_eq!(
            config.auth_scheme_for_url("https://unknown.com/pkg"),
            AuthScheme::Bearer
        );
    }

    #[test]
    fn authorization_header_bearer() {
        let mut config = make_config();
        config.default_registry_auth_token = Some("my-token".to_string());

        let header = config
            .authorization_header_for_url("https://registry.npmjs.org/pkg")
            .unwrap();
        assert_eq!(header, "Bearer my-token");
    }

    #[test]
    fn authorization_header_returns_none_without_token() {
        let config = make_config();
        assert!(
            config
                .authorization_header_for_url("https://registry.npmjs.org/pkg")
                .is_none()
        );
    }

    #[test]
    fn derived_directories() {
        let config = make_config();
        assert_eq!(config.packages_dir(), PathBuf::from("/tmp/data/packages"));
        assert_eq!(config.metadata_dir(), PathBuf::from("/tmp/data/metadata"));
        assert_eq!(config.global_dir(), PathBuf::from("/tmp/data/global"));
        assert_eq!(config.global_bin_dir(), PathBuf::from("/tmp/data/bin"));
    }
}
