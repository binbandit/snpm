use crate::config::SnpmConfig;

use std::path::PathBuf;

impl SnpmConfig {
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
}
