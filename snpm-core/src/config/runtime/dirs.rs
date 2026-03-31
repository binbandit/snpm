use directories::ProjectDirs;
use std::env;
use std::path::PathBuf;

pub(super) fn resolve_home_dirs() -> (PathBuf, PathBuf) {
    if let Ok(home) = env::var("SNPM_HOME") {
        let base = PathBuf::from(home);
        return (base.join("cache"), base.join("data"));
    }

    if let Some(dirs) = ProjectDirs::from("io", "snpm", "snpm") {
        return (
            dirs.cache_dir().to_path_buf(),
            dirs.data_local_dir().to_path_buf(),
        );
    }

    let fallback = PathBuf::from(".snpm");
    (fallback.join("cache"), fallback.join("data"))
}
