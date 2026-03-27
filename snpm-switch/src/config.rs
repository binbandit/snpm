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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_dir_uses_env_var() {
        unsafe { std::env::set_var("SNPM_SWITCH_HOME", "/custom/switch") };
        let result = switch_dir().unwrap();
        assert_eq!(result, PathBuf::from("/custom/switch"));
        unsafe { std::env::remove_var("SNPM_SWITCH_HOME") };
    }

    #[test]
    fn versions_dir_is_subdir_of_switch() {
        let switch = switch_dir().unwrap();
        let versions = versions_dir().unwrap();
        assert_eq!(versions, switch.join("versions"));
    }

    #[test]
    fn download_base_url_default() {
        unsafe { std::env::remove_var("SNPM_DOWNLOAD_URL") };
        let url = download_base_url();
        assert!(url.contains("github.com"));
        assert!(url.contains("snpm"));
    }

    #[test]
    fn download_base_url_custom() {
        unsafe { std::env::set_var("SNPM_DOWNLOAD_URL", "https://custom.cdn.com/releases") };
        let url = download_base_url();
        assert_eq!(url, "https://custom.cdn.com/releases");
        unsafe { std::env::remove_var("SNPM_DOWNLOAD_URL") };
    }
}
