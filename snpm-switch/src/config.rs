use std::path::PathBuf;

pub fn switch_dir() -> PathBuf {
    if let Ok(home) = std::env::var("SNPM_SWITCH_HOME") {
        return PathBuf::from(home);
    }

    dirs::data_local_dir()
        .map(|d| d.join("snpm-switch"))
        .unwrap_or_else(|| PathBuf::from(".snpm-switch"))
}

pub fn versions_dir() -> PathBuf {
    switch_dir().join("versions")
}

pub fn download_base_url() -> String {
    std::env::var("SNPM_DOWNLOAD_URL")
        .unwrap_or_else(|_| "https://github.com/binbandit/snpm/releases/download".to_string())
}
