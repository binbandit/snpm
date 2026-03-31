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
    use std::ffi::OsString;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: Option<&str>) -> Self {
            let previous = std::env::var_os(key);

            match value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }

            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => unsafe { std::env::set_var(self.key, value) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    #[test]
    fn switch_dir_uses_env_var() {
        let _lock = env_lock();
        let _guard = EnvVarGuard::set("SNPM_SWITCH_HOME", Some("/custom/switch"));
        let result = switch_dir().unwrap();
        assert_eq!(result, PathBuf::from("/custom/switch"));
    }

    #[test]
    fn versions_dir_is_subdir_of_switch() {
        let _lock = env_lock();
        let _guard = EnvVarGuard::set("SNPM_SWITCH_HOME", None);
        let switch = switch_dir().unwrap();
        let versions = versions_dir().unwrap();
        assert_eq!(versions, switch.join("versions"));
    }

    #[test]
    fn download_base_url_default() {
        let _lock = env_lock();
        let _guard = EnvVarGuard::set("SNPM_DOWNLOAD_URL", None);
        let url = download_base_url();
        assert!(url.contains("github.com"));
        assert!(url.contains("snpm"));
    }

    #[test]
    fn download_base_url_custom() {
        let _lock = env_lock();
        let _guard = EnvVarGuard::set("SNPM_DOWNLOAD_URL", Some("https://custom.cdn.com/releases"));
        let url = download_base_url();
        assert_eq!(url, "https://custom.cdn.com/releases");
    }
}
