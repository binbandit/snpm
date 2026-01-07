use directories::{BaseDirs, ProjectDirs};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthScheme {
    Bearer,
    Basic,
}
use std::collections::{BTreeMap, BTreeSet};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

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

fn expand_env_vars(s: &str) -> String {
    use std::env;

    let mut out = String::new();
    let mut i = 0;
    let bytes = s.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'$' {
            if i + 1 < bytes.len()
                && bytes[i + 1] == b'{'
                && let Some(end) = s[i + 2..].find('}')
            {
                let var = &s[i + 2..i + 2 + end];
                let val = env::var(var).unwrap_or_default();
                out.push_str(&val);
                i += 2 + end + 1;
                continue;
            }

            let mut j = i + 1;
            while j < bytes.len()
                && (bytes[j] == b'_' || (bytes[j] as char).is_ascii_alphanumeric())
            {
                j += 1;
            }

            let var = &s[i + 1..j];
            if !var.is_empty() {
                let val = env::var(var).unwrap_or_default();
                out.push_str(&val);
                i = j;
                continue;
            }

            out.push('$');
            i += 1;
            continue;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }

    out
}

fn normalize_registry_url(value: &str) -> String {
    let mut v = if value.starts_with("//") {
        format!("https:{}", value)
    } else {
        value.to_string()
    };

    if v.ends_with('/') {
        v.pop();
    }

    let (scheme, rest) = if let Some(r) = v.strip_prefix("https://") {
        ("https", r)
    } else if let Some(r) = v.strip_prefix("http://") {
        ("http", r)
    } else if let Some(r) = v.strip_prefix("//") {
        ("https", r)
    } else {
        return v;
    };

    let mut parts = rest.splitn(2, '/');
    let hostport = parts.next().unwrap_or("").to_ascii_lowercase();
    let suffix = parts.next().unwrap_or("");

    let mut host = hostport.clone();
    if let Some((h, p)) = hostport.split_once(':') {
        let default_https = scheme == "https" && p == "443";
        let default_http = scheme == "http" && p == "80";
        if default_https || default_http {
            host = h.to_string();
        }
    }

    if suffix.is_empty() {
        format!("{}://{}", scheme, host)
    } else {
        format!("{}://{}/{}", scheme, host, suffix)
    }
}

fn read_allow_scripts_from_env() -> BTreeSet<String> {
    let mut set = BTreeSet::new();

    if let Ok(value) = env::var("SNPM_ALLOW_SCRIPTS") {
        for part in value.split(',') {
            let name = part.trim();
            if !name.is_empty() {
                set.insert(name.to_string());
            }
        }
    }

    set
}

fn read_min_package_age_from_env() -> Option<u32> {
    if let Ok(value) = env::var("SNPM_MIN_PACKAGE_AGE_DAYS") {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }

        if let Ok(parsed) = trimmed.parse::<u32>()
            && parsed > 0
        {
            return Some(parsed);
        }
    }

    None
}

fn read_min_package_cache_age_from_env() -> Option<u32> {
    if let Ok(value) = env::var("SNPM_MIN_PACKAGE_CACHE_AGE_DAYS") {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Some(7);
        }

        if let Ok(parsed) = trimmed.parse::<u32>()
            && parsed > 0
        {
            return Some(parsed);
        }
    }

    Some(7)
}

fn read_registry_config() -> (
    String,
    BTreeMap<String, String>,
    BTreeMap<String, String>,
    Option<String>,
    Option<HoistingMode>,
    bool,
) {
    let mut default_registry = "https://registry.npmjs.org/".to_string();
    let mut scoped = BTreeMap::new();
    let mut registry_auth = BTreeMap::new();
    let mut default_auth_token = None;
    let mut hoisting = None;
    let mut rc_default_auth_basic = false;

    if let Some(base) = BaseDirs::new() {
        let home = base.home_dir();
        let rc_files = [".snpmrc", ".npmrc", ".pnpmrc"];

        for rc_name in rc_files.iter() {
            let path = home.join(rc_name);
            apply_rc_file(
                &path,
                &mut default_registry,
                &mut scoped,
                &mut registry_auth,
                &mut default_auth_token,
                &mut hoisting,
                &mut rc_default_auth_basic,
            );
        }
    }

    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let rc_files = [".snpmrc", ".npmrc", ".pnpmrc"];

    // Walk up parent directories to root to respect rc precedence across nested workspaces
    let mut directory = Some(cwd.clone());
    while let Some(dir) = directory {
        for rc_name in rc_files.iter() {
            let path = dir.join(rc_name);

            apply_rc_file(
                &path,
                &mut default_registry,
                &mut scoped,
                &mut registry_auth,
                &mut default_auth_token,
                &mut hoisting,
                &mut rc_default_auth_basic,
            );
        }

        directory = dir.parent().map(|p| p.to_path_buf());
        if directory.as_ref().map(|d| d == &dir).unwrap_or(true) {
            break;
        }
    }

    (
        default_registry,
        scoped,
        registry_auth,
        default_auth_token,
        hoisting,
        rc_default_auth_basic,
    )
}

fn apply_rc_file(
    path: &Path,
    default_registry: &mut String,
    scoped: &mut BTreeMap<String, String>,
    registry_auth: &mut BTreeMap<String, String>,
    default_auth_token: &mut Option<String>,
    hoisting: &mut Option<HoistingMode>,
    default_auth_basic: &mut bool,
) {
    if !path.is_file() {
        return;
    }

    if let Ok(data) = fs::read_to_string(path) {
        for line in data.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }

            if let Some(equals_index) = trimmed.find('=') {
                let (key, value) = trimmed.split_at(equals_index);
                let key = key.trim();
                let mut value = expand_env_vars(value[1..].trim());

                if value.ends_with('/') && !key.starts_with("//") {
                    value.pop();
                }

                if key == "registry" {
                    if !value.is_empty() {
                        *default_registry = normalize_registry_url(&value);
                    }
                } else if key == "snpm-hoist" || key == "snpm.hoist" || key == "snpm_hoist" {
                    if let Some(mode) = HoistingMode::from_str(&value) {
                        *hoisting = Some(mode);
                    }
                } else if let Some(scope) = key.strip_suffix(":registry") {
                    let scope = scope.trim();
                    if !scope.is_empty() && !value.is_empty() {
                        scoped.insert(scope.to_string(), normalize_registry_url(&value));
                    }
                } else if let Some(rest) = key.strip_prefix("//") {
                    let host_and_path = if let Some(prefix) = rest.strip_suffix("/:_authToken") {
                        prefix
                    } else if let Some(prefix) = rest.strip_suffix(":_authToken") {
                        prefix
                    } else {
                        ""
                    };

                    if !host_and_path.is_empty() {
                        let raw_host = host_and_path
                            .split('/')
                            .next()
                            .unwrap_or("")
                            .trim()
                            .trim_end_matches('/');

                        // Normalize host:
                        // - Lowercase
                        // - Strip default ports (:443, :80) to match PNPM behavior
                        let mut host = raw_host.to_ascii_lowercase();
                        if let Some((split_host, split_port)) = host.split_once(':')
                            && (split_port == "443" || split_port == "80")
                        {
                            host = split_host.to_string();
                        }

                        if !host.is_empty() && !value.is_empty() {
                            registry_auth.insert(host.to_string(), value);
                        }
                    }
                } else if key == "_authToken" {
                    if !value.is_empty() {
                        *default_auth_token = Some(value);
                    }
                } else if key == "_auth" {
                    // Support legacy _auth entries (basic auth). Store as default token and mark scheme
                    if !value.is_empty() {
                        *default_auth_token = Some(value);
                        *default_auth_basic = true;
                    }
                } else if key == "always-auth" || key == "always_auth" || key == "always.auth" {
                    let value = value.trim().to_ascii_lowercase();
                    let on = matches!(value.as_str(), "1" | "true" | "yes" | "y" | "on");
                    if on {
                        // This flag is applied in from_env() defaults
                        // Kept here for symmetry with pnpm configs
                    }
                }
            }
        }
    }
}

pub(crate) fn host_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let is_https = trimmed.starts_with("https://");
    let is_http = trimmed.starts_with("http://");

    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);

    let hostport = without_scheme.split('/').next().unwrap_or("").trim();

    if hostport.is_empty() {
        return None;
    }

    let mut host = hostport.to_ascii_lowercase();

    if let Some((split_host, split_port)) = host.split_once(':') {
        let default_https = is_https && split_port == "443";
        let default_http = is_http && split_port == "80";
        if default_https || default_http {
            host = split_host.to_string();
        }
    }

    if host.is_empty() { None } else { Some(host) }
}
