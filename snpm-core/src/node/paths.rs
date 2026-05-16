use crate::config::SnpmConfig;

use std::path::PathBuf;

impl SnpmConfig {
    pub fn node_root_dir(&self) -> PathBuf {
        self.data_dir.join("node")
    }

    pub fn node_versions_dir(&self) -> PathBuf {
        self.node_root_dir().join("versions")
    }

    pub fn node_aliases_dir(&self) -> PathBuf {
        self.node_root_dir().join("aliases")
    }

    pub fn node_current_pointer_path(&self) -> PathBuf {
        self.node_root_dir().join("current")
    }

    pub fn node_index_cache_path(&self) -> PathBuf {
        self.cache_dir.join("node").join("index.json")
    }

    pub fn node_version_dir(&self, normalized_version: &str) -> PathBuf {
        self.node_versions_dir().join(normalized_version)
    }

    pub fn node_version_bin_dir(&self, normalized_version: &str) -> PathBuf {
        let version_dir = self.node_version_dir(normalized_version);
        if cfg!(windows) {
            version_dir
        } else {
            version_dir.join("bin")
        }
    }
}
