use directories::{BaseDirs, ProjectDirs};
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
    pub default_registry: String,
    pub scoped_registries: BTreeMap<String, String>,
    pub registry_auth: BTreeMap<String, String>,
    pub default_registry_auth_token: Option<String>,
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

        let (rc_default_registry, scoped_registries, registry_auth, rc_default_auth_token) =
            read_registry_config();

        let mut default_registry = rc_default_registry;
        let mut default_registry_auth_token = rc_default_auth_token;

        if let Ok(value) =
            env::var("NPM_CONFIG_REGISTRY").or_else(|_| env::var("npm_config_registry"))
        {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                let new_default = trimmed.to_string();

                let rc_host = host_from_url(&default_registry);
                let new_host = host_from_url(&new_default);

                if rc_host != new_host {
                    default_registry_auth_token = None;
                }

                default_registry = new_default;
            }
        }

        if let Ok(token) = env::var("NODE_AUTH_TOKEN")
            .or_else(|_| env::var("NPM_TOKEN"))
            .or_else(|_| env::var("SNPM_AUTH_TOKEN"))
        {
            let trimmed = token.trim();
            if !trimmed.is_empty() {
                default_registry_auth_token = Some(trimmed.to_string());
            }
        }

        SnpmConfig {
            cache_dir,
            data_dir,
            allow_scripts,
            min_package_age_days,
            default_registry,
            scoped_registries,
            registry_auth,
            default_registry_auth_token,
        }
    }

    pub fn packages_dir(&self) -> PathBuf {
        self.data_dir.join("packages")
    }

    pub fn auth_token_for_url(&self, url: &str) -> Option<&str> {
        let host = host_from_url(url)?;

        if let Some(token) = self.registry_auth.get(&host) {
            return Some(token.as_str());
        }

        if let Some(default_host) = host_from_url(&self.default_registry) {
            if host == default_host {
                if let Some(token) = self.default_registry_auth_token.as_ref() {
                    return Some(token.as_str());
                }
            }
        }

        None
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

        if let Ok(parsed) = trimmed.parse::<u32>() {
            if parsed > 0 {
                return Some(parsed);
            }
        }
    }

    None
}

fn read_registry_config() -> (
    String,
    BTreeMap<String, String>,
    BTreeMap<String, String>,
    Option<String>,
) {
    let mut default_registry = "https://registry.npmjs.org/".to_string();
    let mut scoped = BTreeMap::new();
    let mut registry_auth = BTreeMap::new();
    let mut default_auth_token = None;

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
            );
        }
    }

    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let rc_files = [".snpmrc", ".npmrc", ".pnpmrc"];

    for rc_name in rc_files.iter() {
        let path = cwd.join(rc_name);

        apply_rc_file(
            &path,
            &mut default_registry,
            &mut scoped,
            &mut registry_auth,
            &mut default_auth_token,
        );
    }

    (default_registry, scoped, registry_auth, default_auth_token)
}

fn apply_rc_file(
    path: &Path,
    default_registry: &mut String,
    scoped: &mut BTreeMap<String, String>,
    registry_auth: &mut BTreeMap<String, String>,
    default_auth_token: &mut Option<String>,
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

            if let Some(eq_idx) = trimmed.find('=') {
                let (key, value) = trimmed.split_at(eq_idx);
                let key = key.trim();
                let mut value = value[1..].trim().to_string();

                if value.ends_with('/') && !key.starts_with("//") {
                    value.pop();
                }

                if key == "registry" {
                    if !value.is_empty() {
                        *default_registry = value;
                    }
                } else if let Some(scope) = key.strip_suffix(":registry") {
                    let scope = scope.trim();
                    if !scope.is_empty() && !value.is_empty() {
                        scoped.insert(scope.to_string(), value);
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
                        let host = host_and_path
                            .split('/')
                            .next()
                            .unwrap_or("")
                            .trim()
                            .trim_end_matches('/');

                        if !host.is_empty() && !value.is_empty() {
                            registry_auth.insert(host.to_string(), value);
                        }
                    }
                } else if key == "_authToken" {
                    if !value.is_empty() {
                        *default_auth_token = Some(value);
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

    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);

    let host = without_scheme.split('/').next().unwrap_or("").trim();

    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}
