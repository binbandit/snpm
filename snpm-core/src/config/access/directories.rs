use crate::config::SnpmConfig;

use std::path::PathBuf;

impl SnpmConfig {
    pub fn packages_dir(&self) -> PathBuf {
        self.data_dir.join("packages")
    }

    pub fn virtual_store_dir(&self) -> PathBuf {
        self.data_dir.join("virtual-store")
    }

    pub fn side_effects_cache_dir(&self) -> PathBuf {
        self.data_dir.join("side-effects-v1")
    }

    pub fn metadata_dir(&self) -> PathBuf {
        self.data_dir.join("metadata")
    }

    pub fn store_residency_index_path(&self) -> PathBuf {
        self.metadata_dir().join("store-residency-v1.bin")
    }

    pub fn global_dir(&self) -> PathBuf {
        self.data_dir.join("global")
    }

    pub fn global_bin_dir(&self) -> PathBuf {
        self.data_dir.join("bin")
    }
}
