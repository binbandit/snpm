use directories::ProjectDirs;
use std::{collections::BTreeSet, env, path::PathBuf};

#[derive(Debug, Clone)]
pub struct SnpmConfig {
    pub cache_dir: PathBuf,
    pub data_dir: PathBuf,
    pub allow_scripts: BTreeSet<String>,
    pub min_package_age_days: Option<u32>,
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

        SnpmConfig {
            cache_dir,
            data_dir,
            allow_scripts,
            min_package_age_days,
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
