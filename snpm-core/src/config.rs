use directories::ProjectDirs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SnpmConfig {
    pub cache_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl SnpmConfig {
    pub fn from_env() -> Self {
        let dirs = ProjectDirs::from("io", "snpm", "snpm");
        match dirs {
            Some(dirs) => SnpmConfig {
                cache_dir: dirs.cache_dir().to_path_buf(),
                data_dir: dirs.data_local_dir().to_path_buf(),
            },
            None => {
                let fallback = PathBuf::from(".snpm");
                SnpmConfig {
                    cache_dir: fallback.join("cache"),
                    data_dir: fallback.join("data"),
                }
            }
        }
    }

    pub fn packages_dir(&self) -> PathBuf {
        self.data_dir.join("packages")
    }
}
