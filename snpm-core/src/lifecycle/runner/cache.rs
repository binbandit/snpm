use super::remote_cache::RemoteCache;
use crate::copying::clone_or_copy_file;
use crate::linker::fs::{symlink_dir_entry, symlink_file_entry};
use crate::node;
use crate::{Result, SnpmConfig, SnpmError};

use sha2::{Digest, Sha512};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

const SIDE_EFFECTS_CACHE_MARKER: &str = ".snpm-side-effects-cache";
const SIDE_EFFECTS_TMP_PREFIX: &str = ".tmp-side-effects-";

pub(super) enum SideEffectsCacheRestore {
    Miss,
    Restored,
    AlreadyApplied,
}

pub(super) struct SideEffectsCacheEntry {
    input_hash: String,
    path: PathBuf,
    /// `<platform>/<name>@<version>/<input_hash>.tar.gz` — the key used
    /// for the remote backend. Empty when no remote is configured (we
    /// avoid the format! work in the common case).
    remote_key: String,
    remote: Option<RemoteCache>,
}

impl SideEffectsCacheEntry {
    pub(super) fn new(
        config: &SnpmConfig,
        name: &str,
        version: &str,
        package_dir: &Path,
    ) -> Result<Self> {
        let input_hash = match read_marker(package_dir) {
            Some(hash) => hash,
            None => hash_dir(package_dir)?,
        };
        let safe_name = name.replace('/', "__");
        // Native modules link against a specific Node ABI; restoring an
        // artifact built under one Node major into a project pinned to a
        // different major produces opaque "abi mismatch" or NAPI version
        // errors at require-time. Partition the cache slot per Node major
        // so the per-package side-effects survive correctly across
        // project Node-version changes. Pure-JS packages pay a tiny disk
        // cost (one extra slot per Node major they're built under) but
        // never miss-restore; native packages stay correct.
        let engine = node_engine_tag(config, package_dir);
        let platform = format!(
            "{}-{}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH,
            engine
        );

        let remote = RemoteCache::from_config(config);
        let remote_key = if remote.is_some() {
            format!("{platform}/{safe_name}@{version}/{input_hash}.tar.gz")
        } else {
            String::new()
        };

        Ok(Self {
            path: config
                .side_effects_cache_dir()
                .join(&platform)
                .join(format!("{safe_name}@{version}"))
                .join(&input_hash),
            input_hash,
            remote_key,
            remote,
        })
    }

    pub(super) fn restore_if_available(
        &self,
        package_dir: &Path,
    ) -> Result<SideEffectsCacheRestore> {
        if marker_matches(package_dir, &self.input_hash) && self.path.is_dir() {
            return Ok(SideEffectsCacheRestore::AlreadyApplied);
        }

        if self.path.is_dir() {
            copy_dir(&self.path, package_dir)?;
            return Ok(SideEffectsCacheRestore::Restored);
        }

        // Local miss — try the remote cache. On hit, restore directly
        // into the package dir AND populate the local slot so the next
        // install short-circuits without re-downloading.
        if let Some(remote) = &self.remote
            && remote.try_restore(&self.remote_key, package_dir)
        {
            // Promote to local for subsequent installs.
            if let Err(error) = self.save_local_from(package_dir) {
                crate::console::verbose(&format!(
                    "remote cache restored {} but local promotion failed: {}",
                    self.remote_key, error
                ));
            }
            write_marker(package_dir, &self.input_hash)?;
            return Ok(SideEffectsCacheRestore::Restored);
        }

        Ok(SideEffectsCacheRestore::Miss)
    }

    fn save_local_from(&self, package_dir: &Path) -> Result<()> {
        if self.path.is_dir() {
            return Ok(());
        }
        let parent = self.path.parent().ok_or_else(|| SnpmError::Internal {
            reason: format!(
                "side-effects cache path has no parent: {}",
                self.path.display()
            ),
        })?;
        fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;
        let tmp_dir = parent.join(format!(
            "{SIDE_EFFECTS_TMP_PREFIX}{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        if tmp_dir.exists() {
            remove_path(&tmp_dir)?;
        }
        copy_dir(package_dir, &tmp_dir)?;
        match fs::rename(&tmp_dir, &self.path) {
            Ok(()) => Ok(()),
            Err(_) if self.path.is_dir() => {
                remove_path(&tmp_dir).ok();
                Ok(())
            }
            Err(source) => {
                remove_path(&tmp_dir).ok();
                Err(SnpmError::WriteFile {
                    path: self.path.clone(),
                    source,
                })
            }
        }
    }

    pub(super) fn save(&self, package_dir: &Path) -> Result<()> {
        if self.path.is_dir() {
            write_marker(package_dir, &self.input_hash)?;
            // The local slot already existed: this artifact was either
            // produced by an earlier run (which uploaded it then) or
            // promoted from a remote hit. Re-PUTting the whole tree on
            // every warm install would multiply upload traffic across a
            // fleet for no benefit.
            return Ok(());
        }

        let parent = self.path.parent().ok_or_else(|| SnpmError::Internal {
            reason: format!(
                "side-effects cache path has no parent: {}",
                self.path.display()
            ),
        })?;
        fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;

        write_marker(package_dir, &self.input_hash)?;

        let tmp_dir = parent.join(format!(
            "{SIDE_EFFECTS_TMP_PREFIX}{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));

        if tmp_dir.exists() {
            remove_path(&tmp_dir)?;
        }

        copy_dir(package_dir, &tmp_dir)?;

        let local_save = match fs::rename(&tmp_dir, &self.path) {
            Ok(()) => Ok(()),
            Err(_source) if self.path.is_dir() => {
                remove_path(&tmp_dir).ok();
                Ok(())
            }
            Err(source) => {
                remove_path(&tmp_dir).ok();
                Err(SnpmError::WriteFile {
                    path: self.path.clone(),
                    source,
                })
            }
        };

        local_save?;
        self.push_remote(package_dir);
        Ok(())
    }

    fn push_remote(&self, package_dir: &Path) {
        if let Some(remote) = &self.remote {
            remote.try_upload(&self.remote_key, package_dir);
        }
    }
}

/// Returns a cache-key fragment like `node18` that identifies the Node
/// major the package's scripts will run under. Preference order:
///   1. The pinned version from `.node-version`/`.nvmrc`/`engines.node`,
///      parsed for its major. This is the user's stated intent; using it
///      before resolution means we partition correctly even when the
///      pinned Node hasn't been installed yet (auto-install will install
///      it before scripts run).
///   2. The resolved active Node version (if pin is unresolvable but a
///      Node is installed).
///   3. The Node version on PATH (`node --version`).
///   4. Fallback sentinel `node-unknown` — partitions unknown-version
///      installs into their own slot rather than risking a wrong-ABI
///      restore.
fn node_engine_tag(config: &SnpmConfig, package_dir: &Path) -> String {
    if let Some(major) = pinned_node_major(package_dir) {
        return format!("node{major}");
    }

    if let Some(active) = node::exec::active_for_project_offline(config, package_dir)
        .ok()
        .flatten()
        && let Some(major) = parse_node_major(&active.version)
    {
        return format!("node{major}");
    }

    if let Some(major) = process_node_major() {
        return format!("node{major}");
    }

    "node-unknown".to_string()
}

fn pinned_node_major(start: &Path) -> Option<u32> {
    let pin = node::discover::discover_pinned(start).ok().flatten()?;
    parse_node_major(&pin.spec)
}

fn process_node_major() -> Option<u32> {
    static CACHED: OnceLock<Option<u32>> = OnceLock::new();
    *CACHED.get_or_init(|| {
        let output = Command::new("node").arg("--version").output().ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8(output.stdout).ok()?;
        parse_node_major(&text)
    })
}

fn parse_node_major(version: &str) -> Option<u32> {
    let trimmed = version.trim().trim_start_matches('v');
    let major = trimmed.split('.').next()?;
    major.parse::<u32>().ok()
}

fn marker_matches(package_dir: &Path, expected: &str) -> bool {
    read_marker(package_dir).is_some_and(|value| value == expected)
}

fn read_marker(package_dir: &Path) -> Option<String> {
    let marker = fs::read_to_string(package_dir.join(SIDE_EFFECTS_CACHE_MARKER)).ok()?;
    let marker = marker.trim();

    (marker.len() == 128 && marker.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| marker.to_ascii_lowercase())
}

fn write_marker(package_dir: &Path, input_hash: &str) -> Result<()> {
    let path = package_dir.join(SIDE_EFFECTS_CACHE_MARKER);
    fs::write(&path, input_hash).map_err(|source| SnpmError::WriteFile { path, source })
}

fn hash_dir(package_dir: &Path) -> Result<String> {
    let mut hasher = Sha512::new();
    hash_dir_inner(package_dir, package_dir, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}

fn hash_dir_inner(base: &Path, current: &Path, hasher: &mut Sha512) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(current)
        .map_err(|source| SnpmError::ReadFile {
            path: current.to_path_buf(),
            source,
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| SnpmError::ReadFile {
            path: current.to_path_buf(),
            source,
        })?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) == Some(SIDE_EFFECTS_CACHE_MARKER) {
            continue;
        }

        let relative = path
            .strip_prefix(base)
            .map_err(|source| SnpmError::Io {
                path: path.clone(),
                source: std::io::Error::other(source),
            })?
            .to_string_lossy()
            .replace('\\', "/");

        let metadata = fs::symlink_metadata(&path).map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })?;

        hasher.update(relative.as_bytes());

        if metadata.file_type().is_symlink() {
            hasher.update(b"\0symlink\0");
            let target = fs::read_link(&path).map_err(|source| SnpmError::ReadFile {
                path: path.clone(),
                source,
            })?;
            hasher.update(target.to_string_lossy().as_bytes());
            continue;
        }

        if metadata.is_dir() {
            hasher.update(b"\0dir\0");
            hash_dir_inner(base, &path, hasher)?;
            continue;
        }

        hasher.update(b"\0file\0");
        let bytes = fs::read(&path).map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })?;
        hasher.update(&bytes);
    }

    Ok(())
}

fn copy_dir(source: &Path, destination: &Path) -> Result<()> {
    if destination.symlink_metadata().is_ok() {
        remove_path(destination)?;
    }

    fs::create_dir_all(destination).map_err(|source_err| SnpmError::WriteFile {
        path: destination.to_path_buf(),
        source: source_err,
    })?;

    copy_dir_inner(source, source, destination)
}

fn copy_dir_inner(base: &Path, current: &Path, destination_root: &Path) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(current)
        .map_err(|source| SnpmError::ReadFile {
            path: current.to_path_buf(),
            source,
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|source| SnpmError::ReadFile {
            path: current.to_path_buf(),
            source,
        })?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let relative = path.strip_prefix(base).map_err(|source| SnpmError::Io {
            path: path.clone(),
            source: std::io::Error::other(source),
        })?;
        let destination = destination_root.join(relative);
        let metadata = fs::symlink_metadata(&path).map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })?;

        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path).map_err(|source| SnpmError::ReadFile {
                path: path.clone(),
                source,
            })?;
            create_symlink_like(&path, &target, &destination)?;
            continue;
        }

        if metadata.is_dir() {
            fs::create_dir_all(&destination).map_err(|source| SnpmError::WriteFile {
                path: destination.clone(),
                source,
            })?;
            copy_dir_inner(base, &path, destination_root)?;
            continue;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        clone_or_copy_file(&path, &destination).map_err(|source| SnpmError::WriteFile {
            path: destination.clone(),
            source,
        })?;
    }

    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let result = if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };

    result.map_err(|source| SnpmError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}

fn create_symlink_like(source: &Path, target: &Path, destination: &Path) -> Result<()> {
    let target_metadata = fs::metadata(source).map_err(|source_err| SnpmError::ReadFile {
        path: source.to_path_buf(),
        source: source_err,
    })?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let link_result = if target_metadata.is_dir() {
        symlink_dir_entry(target, destination)
    } else {
        symlink_file_entry(target, destination)
    };

    link_result.map_err(|source| SnpmError::WriteFile {
        path: destination.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{SideEffectsCacheEntry, SideEffectsCacheRestore};
    use crate::config::SnpmConfig;

    
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_config(data_dir: PathBuf) -> SnpmConfig {
    SnpmConfig {
        cache_dir: data_dir.join("cache"),
        data_dir,
        ..SnpmConfig::for_tests()
    }
}

    #[test]
    fn saves_and_restores_side_effects() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));
        let package_dir = dir.path().join("package");

        fs::create_dir_all(&package_dir).unwrap();
        fs::write(
            package_dir.join("package.json"),
            r#"{"name":"esbuild","version":"1.0.0"}"#,
        )
        .unwrap();
        fs::write(package_dir.join("built.txt"), "built").unwrap();

        let entry = SideEffectsCacheEntry::new(&config, "esbuild", "1.0.0", &package_dir).unwrap();
        entry.save(&package_dir).unwrap();

        fs::remove_dir_all(&package_dir).unwrap();
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(
            package_dir.join("package.json"),
            r#"{"name":"esbuild","version":"1.0.0"}"#,
        )
        .unwrap();

        let restore = entry.restore_if_available(&package_dir).unwrap();

        assert!(matches!(restore, SideEffectsCacheRestore::Restored));
        assert_eq!(
            fs::read_to_string(package_dir.join("built.txt")).unwrap(),
            "built"
        );
    }

    #[test]
    fn cache_path_partitions_by_node_major() {
        // Pinning two different Node majors via .node-version must
        // produce cache slots whose `<platform>` segment differs (the
        // input_hash differs too because .node-version is part of the
        // dir, but the engine segment is what protects against ABI
        // mismatch on restore). The check on the resulting path
        // proves the engine tag flows into the slot layout.
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));

        let pkg_18 = dir.path().join("pkg-18");
        fs::create_dir_all(&pkg_18).unwrap();
        fs::write(pkg_18.join(".node-version"), "v18.20.4").unwrap();
        let entry_18 = SideEffectsCacheEntry::new(&config, "native-mod", "1.0.0", &pkg_18).unwrap();

        let pkg_22 = dir.path().join("pkg-22");
        fs::create_dir_all(&pkg_22).unwrap();
        fs::write(pkg_22.join(".node-version"), "v22.1.0").unwrap();
        let entry_22 = SideEffectsCacheEntry::new(&config, "native-mod", "1.0.0", &pkg_22).unwrap();

        assert!(
            entry_18.path.to_string_lossy().contains("node18"),
            "expected `node18` segment in path, got {}",
            entry_18.path.display()
        );
        assert!(
            entry_22.path.to_string_lossy().contains("node22"),
            "expected `node22` segment in path, got {}",
            entry_22.path.display()
        );

        let platform_18 = entry_18.path.parent().and_then(|p| p.parent()).unwrap();
        let platform_22 = entry_22.path.parent().and_then(|p| p.parent()).unwrap();
        assert_ne!(
            platform_18, platform_22,
            "the per-platform/engine slot must differ across Node majors"
        );
    }

    #[test]
    fn parse_node_major_handles_v_prefix() {
        assert_eq!(super::parse_node_major("v20.10.0"), Some(20));
        assert_eq!(super::parse_node_major("18.19.0"), Some(18));
        assert_eq!(super::parse_node_major(" v22.1.0\n"), Some(22));
        assert_eq!(super::parse_node_major(""), None);
        assert_eq!(super::parse_node_major("not-a-version"), None);
    }

    #[test]
    fn remote_cache_restore_promotes_to_local() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::{Arc, Mutex};
        use std::thread;

        let dir = tempdir().unwrap();
        let mut config = make_config(dir.path().join("data"));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        config.remote_cache_url = Some(format!("http://{addr}"));

        // Prepare a "remote" payload by packing a directory locally.
        let staged = dir.path().join("staged");
        fs::create_dir_all(&staged).unwrap();
        fs::write(staged.join("built-by-remote.txt"), "from-remote").unwrap();
        let archive = super::super::remote_cache::pack_dir(&staged).unwrap();
        let storage: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(archive));
        let storage_for_server = storage.clone();

        let server = thread::spawn(move || {
            for stream in listener.incoming().take(1) {
                let mut stream = stream.unwrap();
                let mut buf = [0_u8; 4096];
                let mut headers = Vec::new();
                loop {
                    let read = stream.read(&mut buf).unwrap();
                    if read == 0 {
                        break;
                    }
                    headers.extend_from_slice(&buf[..read]);
                    if headers.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let body = storage_for_server.lock().unwrap().clone();
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(header.as_bytes()).unwrap();
                stream.write_all(&body).unwrap();
            }
        });

        // A "package" with no artifacts yet.
        let package_dir = dir.path().join("package");
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(
            package_dir.join("package.json"),
            r#"{"name":"native-mod","version":"1.0.0"}"#,
        )
        .unwrap();

        let entry =
            SideEffectsCacheEntry::new(&config, "native-mod", "1.0.0", &package_dir).unwrap();
        let restore = entry.restore_if_available(&package_dir).unwrap();
        assert!(matches!(restore, SideEffectsCacheRestore::Restored));
        assert_eq!(
            fs::read_to_string(package_dir.join("built-by-remote.txt")).unwrap(),
            "from-remote",
            "remote cache content should be restored into the package dir"
        );
        // Local cache should now have the slot populated for next time.
        assert!(
            entry.path.is_dir(),
            "local cache slot should be promoted after remote restore"
        );

        server.join().unwrap();
    }
}
