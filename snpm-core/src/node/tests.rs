use crate::SnpmConfig;
use crate::node::{aliases, current, exec, install, uninstall};

use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};
use tempfile::TempDir;

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

struct EnvGuard {
    key: &'static str,
    prior: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let prior = std::env::var_os(key);
        unsafe { std::env::set_var(key, value) };
        Self { key, prior }
    }

    fn unset(key: &'static str) -> Self {
        let prior = std::env::var_os(key);
        unsafe { std::env::remove_var(key) };
        Self { key, prior }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.prior {
            Some(value) => unsafe { std::env::set_var(self.key, value) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}

fn fake_install(config: &SnpmConfig, version: &str) {
    let version_dir = config.node_version_dir(version);
    let bin_dir = if cfg!(windows) {
        version_dir.clone()
    } else {
        version_dir.join("bin")
    };
    fs::create_dir_all(&bin_dir).unwrap();

    let binary_name = if cfg!(windows) { "node.exe" } else { "node" };
    let binary_path = bin_dir.join(binary_name);
    fs::write(&binary_path, b"fake").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&binary_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary_path, permissions).unwrap();
    }

    fs::write(version_dir.join(".snpm_complete"), []).unwrap();
}

fn config_for(home: &Path) -> SnpmConfig {
    let _lock = env_lock();
    let _guard = EnvGuard::set("SNPM_HOME", home.to_str().unwrap());
    SnpmConfig::from_env()
}

#[test]
fn list_versions_returns_newest_first() {
    let temp = TempDir::new().unwrap();
    let config = config_for(temp.path());

    fake_install(&config, "v18.19.0");
    fake_install(&config, "v20.10.0");
    fake_install(&config, "v21.5.0");

    let versions = uninstall::list_installed_versions(&config).unwrap();
    assert_eq!(versions, vec!["v21.5.0", "v20.10.0", "v18.19.0"]);
}

#[test]
fn list_versions_skips_incomplete_dirs() {
    let temp = TempDir::new().unwrap();
    let config = config_for(temp.path());

    fake_install(&config, "v20.10.0");
    let partial = config.node_version_dir("v21.0.0");
    fs::create_dir_all(partial.join("bin")).unwrap();
    fs::write(partial.join("bin").join("node"), b"x").unwrap();

    let versions = uninstall::list_installed_versions(&config).unwrap();
    assert_eq!(versions, vec!["v20.10.0"]);
}

#[test]
fn is_version_installed_requires_marker_and_binary() {
    let temp = TempDir::new().unwrap();
    let config = config_for(temp.path());

    assert!(!uninstall::is_version_installed(&config, "v20.10.0"));
    fake_install(&config, "v20.10.0");
    assert!(uninstall::is_version_installed(&config, "v20.10.0"));
}

#[test]
fn current_pointer_roundtrip() {
    let temp = TempDir::new().unwrap();
    let config = config_for(temp.path());

    assert!(current::read_current(&config).unwrap().is_none());

    current::write_current(&config, "v20.10.0").unwrap();
    assert_eq!(
        current::read_current(&config).unwrap().as_deref(),
        Some("v20.10.0")
    );

    current::clear_current(&config).unwrap();
    assert!(current::read_current(&config).unwrap().is_none());
}

#[test]
fn aliases_can_be_written_listed_and_removed() {
    let temp = TempDir::new().unwrap();
    let config = config_for(temp.path());

    aliases::write_alias(&config, "default", "v20.10.0").unwrap();
    aliases::write_alias(&config, "work", "20").unwrap();

    let entries = aliases::list_aliases(&config).unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["default", "work"]);

    assert!(aliases::remove_alias(&config, "work").unwrap());
    assert!(!aliases::remove_alias(&config, "work").unwrap());
}

#[test]
fn uninstall_clears_current_and_alias_when_matching() {
    let temp = TempDir::new().unwrap();
    let config = config_for(temp.path());

    fake_install(&config, "v20.10.0");
    current::write_current(&config, "v20.10.0").unwrap();
    aliases::write_alias(&config, "default", "v20.10.0").unwrap();

    let summary = uninstall::uninstall_version(&config, "v20.10.0").unwrap();
    assert!(summary.removed_dir.is_some());
    assert!(summary.cleared_current);
    assert_eq!(summary.removed_aliases, vec!["default".to_string()]);
    assert!(current::read_current(&config).unwrap().is_none());
}

#[test]
fn node_bin_dir_resolves_from_node_version_pin() {
    let _lock = env_lock();
    let temp = TempDir::new().unwrap();
    let _home = EnvGuard::set("SNPM_HOME", temp.path().to_str().unwrap());
    let _auto = EnvGuard::unset("SNPM_NODE_AUTO");
    let _override = EnvGuard::unset("SNPM_NODE_BIN_OVERRIDE");

    let config = SnpmConfig::from_env();
    fake_install(&config, "v20.10.0");

    let project = TempDir::new().unwrap();
    fs::write(project.path().join(".node-version"), "20.10.0\n").unwrap();

    let bin_dir = exec::node_bin_dir_for_subprocess(project.path()).unwrap();
    assert_eq!(bin_dir, config.node_version_bin_dir("v20.10.0"));
}

#[test]
fn node_bin_dir_falls_back_to_default_alias() {
    let _lock = env_lock();
    let temp = TempDir::new().unwrap();
    let _home = EnvGuard::set("SNPM_HOME", temp.path().to_str().unwrap());
    let _auto = EnvGuard::unset("SNPM_NODE_AUTO");
    let _override = EnvGuard::unset("SNPM_NODE_BIN_OVERRIDE");

    let config = SnpmConfig::from_env();
    fake_install(&config, "v22.5.1");
    aliases::write_alias(&config, "default", "v22.5.1").unwrap();

    let project = TempDir::new().unwrap();
    let bin_dir = exec::node_bin_dir_for_subprocess(project.path()).unwrap();
    assert_eq!(bin_dir, config.node_version_bin_dir("v22.5.1"));
}

#[test]
fn node_bin_dir_honors_override_env() {
    let _lock = env_lock();
    let temp = TempDir::new().unwrap();
    let _home = EnvGuard::set("SNPM_HOME", temp.path().to_str().unwrap());
    let _override = EnvGuard::set("SNPM_NODE_BIN_OVERRIDE", "/explicit/path");

    let bin_dir = exec::node_bin_dir_for_subprocess(temp.path()).unwrap();
    assert_eq!(bin_dir, std::path::PathBuf::from("/explicit/path"));
}

#[test]
fn node_bin_dir_is_none_when_auto_switch_disabled() {
    let _lock = env_lock();
    let temp = TempDir::new().unwrap();
    let _home = EnvGuard::set("SNPM_HOME", temp.path().to_str().unwrap());
    let _override = EnvGuard::unset("SNPM_NODE_BIN_OVERRIDE");
    let _auto = EnvGuard::set("SNPM_NODE_AUTO", "0");

    let config = SnpmConfig::from_env();
    fake_install(&config, "v20.10.0");

    let project = TempDir::new().unwrap();
    fs::write(project.path().join(".node-version"), "20.10.0\n").unwrap();

    assert!(exec::node_bin_dir_for_subprocess(project.path()).is_none());
}

#[test]
fn install_summary_reports_already_installed_for_existing_version() {
    let temp = TempDir::new().unwrap();
    let config = config_for(temp.path());

    fake_install(&config, "v20.10.0");

    let bin_path = install::node_binary_path(&config.node_version_dir("v20.10.0"));
    assert!(bin_path.is_file());
}
