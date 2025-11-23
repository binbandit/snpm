use directories::ProjectDirs;
use std::collections::BTreeMap;
use std::{collections::BTreeSet, env, path::PathBuf};

#[derive(Debug, Clone)]
pub struct SnpmConfig {
    pub cache_dir: PathBuf,
    pub data_dir: PathBuf,
    pub allow_scripts: BTreeSet<String>,
    pub min_package_age_days: Option<u32>,
    pub default_registry: String,
    pub scoped_registries: BTreeMap<String, String>,
}

impl SnpmConfig {
    pub fn from_env() -> Self {
        let dirs = ProjectDirs::from("io", "snpm", "snpm");

        let (cache_dir, data_dir) = match dirs {
            Some(dirs) => (
                dirs.cache_dir().to_path_buf(),
                dirs.data_local_dir().to_path_buf(),
            ),
            None => {
                let fallback = PathBuf::from(".snpm");
                (fallback.join("cache"), fallback.join("data"))
            }
        };

        let allow_scripts = read_allow_scripts_from_env();
        let min_package_age_days = read_min_package_age_from_env();
        let (default_registry, scoped_registries) = read_registry_config();

        SnpmConfig {
            cache_dir,
            data_dir,
            allow_scripts,
            min_package_age_days,
            default_registry,
            scoped_registries,
        }
    }

    pub fn packages_dir(&self) -> PathBuf {
        self.data_dir.join("packages")
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

fn read_registry_config() -> (String, BTreeMap<String, String>) {
    let mut default_registry = "https://registry.npmjs.org/".to_string();
    let mut scoped = BTreeMap::new();

    // Env override for default registry, npm-compatible name
    if let Ok(value) = env::var("NPM_CONFIG_REGISTRY").or_else(|_| env::var("npm_config_registry"))
    {
        if !value.trim().is_empty() {
            default_registry = value.trim().to_string();
        }
    }

    // Parse rc files in standard precedence: .snpmrc, .npmrc, .pnpmrc in CWD
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let rc_files = [".snpmrc", ".npmrc", ".pnpmrc"];

    for rc_name in rc_files.iter() {
        let path = cwd.join(rc_name);
        if !path.is_file() {
            continue;
        }

        if let Ok(data) = std::fs::read_to_string(&path) {
            for line in data.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }

                if let Some(eq_idx) = trimmed.find('=') {
                    let (key, value) = trimmed.split_at(eq_idx);
                    let key = key.trim();
                    let mut value = value[1..].trim().to_string();

                    if value.ends_with('/') {
                        value.pop();
                    }

                    if key == "registry" {
                        if !value.is_empty() {
                            default_registry = value;
                        }
                    } else if let Some(scope) = key.strip_suffix(":registry") {
                        let scope = scope.trim();
                        if !scope.is_empty() && !value.is_empty() {
                            scoped.insert(scope.to_string(), value);
                        }
                    }
                }
            }
        }
    }

    (default_registry, scoped)
}
