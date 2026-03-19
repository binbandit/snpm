use std::path::PathBuf;

pub fn switch_dir() -> anyhow::Result<PathBuf> {
    if let Ok(home) = std::env::var("SNPM_SWITCH_HOME") {
        return Ok(PathBuf::from(home));
    }

    dirs::data_local_dir()
        .map(|d| d.join("snpm-switch"))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "could not determine data directory; set SNPM_SWITCH_HOME to specify a location"
            )
        })
}

pub fn versions_dir() -> anyhow::Result<PathBuf> {
    Ok(switch_dir()?.join("versions"))
}

pub fn download_base_url() -> String {
    std::env::var("SNPM_DOWNLOAD_URL")
        .unwrap_or_else(|_| "https://github.com/binbandit/snpm/releases/download".to_string())
}
