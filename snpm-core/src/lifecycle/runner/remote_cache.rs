//! Remote side-effects cache backend.
//!
//! Goes alongside the local cache in `cache.rs`: on restore, we first
//! check the local slot, fall back to a remote GET, and (if the remote
//! had it) populate the local slot. After a successful build we write
//! locally and — when allowed — PUT to the remote.
//!
//! Wire format: a single `tar.gz` per cache slot at
//! `<base_url>/<os>-<arch>-node<major>/<name>@<version>/<input_hash>.tar.gz`.
//! Bearer-token auth when `SNPM_REMOTE_CACHE_TOKEN` is set.
//!
//! Errors here are deliberately swallowed (logged via `console::warn`)
//! so a flaky remote never fails an install — the local cache + the
//! script itself remain the source of truth.

use crate::console;
use crate::{Result, SnpmConfig, SnpmError};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::time::Duration;
use tar::{Archive, Builder, Header};

/// Snapshot bytes produced by `pack_dir` and consumed by `unpack_into_dir`.
type ArchiveBytes = Vec<u8>;

pub(super) struct RemoteCache {
    base_url: String,
    auth_token: Option<String>,
    read_only: bool,
}

impl RemoteCache {
    pub(super) fn from_config(config: &SnpmConfig) -> Option<Self> {
        let base_url = config.remote_cache_url.clone()?;
        if base_url.is_empty() {
            return None;
        }
        Some(Self {
            base_url,
            auth_token: config.remote_cache_auth_token.clone(),
            read_only: config.remote_cache_read_only,
        })
    }

    fn client() -> Result<reqwest::blocking::Client> {
        // A separate blocking client — the lifecycle runner is sync
        // (rayon-driven) and synthesizing a tokio runtime per cache
        // call would add real latency to the post-install loop.
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(10))
            .user_agent(concat!("snpm/", env!("CARGO_PKG_VERSION")))
            // Never follow redirects: the bearer token is attached to
            // every request, and a redirecting (or malicious) endpoint
            // must not be able to bounce it to another host.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|source| SnpmError::HttpClient { source })
    }

    fn object_url(&self, key: &str) -> String {
        format!("{}/{}", self.base_url.trim_end_matches('/'), key)
    }

    /// Try to fetch the cached snapshot for `key` and restore it into
    /// `package_dir`. Returns `Ok(true)` if the restore happened,
    /// `Ok(false)` for a clean miss or any non-fatal failure.
    pub(super) fn try_restore(&self, key: &str, package_dir: &Path) -> bool {
        let Ok(client) = Self::client() else {
            return false;
        };
        let url = self.object_url(key);

        let mut request = client.get(&url);
        if let Some(token) = self.auth_token.as_deref() {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = match request.send() {
            Ok(value) => value,
            Err(error) => {
                console::verbose(&format!("remote cache GET {url} failed: {error}"));
                return false;
            }
        };
        let status = response.status();
        if status.as_u16() == 404 {
            return false;
        }
        if !status.is_success() {
            console::verbose(&format!("remote cache GET {url} returned {status}"));
            return false;
        }
        let bytes = match response.bytes() {
            Ok(value) => value.to_vec(),
            Err(error) => {
                console::verbose(&format!("remote cache GET {url} body read failed: {error}"));
                return false;
            }
        };
        if let Err(error) = unpack_into_dir(&bytes, package_dir) {
            console::warn(&format!(
                "remote cache restore from {url} failed during unpack: {error}"
            ));
            return false;
        }
        true
    }

    /// Try to upload `package_dir`'s state under `key`. Best-effort:
    /// failures are logged and swallowed.
    pub(super) fn try_upload(&self, key: &str, package_dir: &Path) {
        if self.read_only {
            return;
        }
        let bytes = match pack_dir(package_dir) {
            Ok(bytes) => bytes,
            Err(error) => {
                console::warn(&format!("remote cache pack for {key} failed: {error}"));
                return;
            }
        };
        let Ok(client) = Self::client() else {
            return;
        };
        let url = self.object_url(key);
        let mut request = client
            .put(&url)
            .header("content-type", "application/gzip")
            .body(bytes);
        if let Some(token) = self.auth_token.as_deref() {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        match request.send() {
            Ok(response) => {
                let status = response.status();
                if !status.is_success() {
                    console::verbose(&format!("remote cache PUT {url} returned {status}"));
                }
            }
            Err(error) => {
                console::verbose(&format!("remote cache PUT {url} failed: {error}"));
            }
        }
    }
}

/// Pack `package_dir`'s contents (including the side-effects marker)
/// into a gzipped tar.
pub(super) fn pack_dir(package_dir: &Path) -> Result<ArchiveBytes> {
    let mut buffer: Vec<u8> = Vec::new();
    {
        let encoder = GzEncoder::new(&mut buffer, Compression::default());
        let mut builder = Builder::new(encoder);
        builder.follow_symlinks(false);
        pack_dir_inner(package_dir, package_dir, &mut builder)?;
        let encoder = builder
            .into_inner()
            .map_err(|source| SnpmError::WriteFile {
                path: package_dir.to_path_buf(),
                source,
            })?;
        encoder.finish().map_err(|source| SnpmError::WriteFile {
            path: package_dir.to_path_buf(),
            source,
        })?;
    }
    Ok(buffer)
}

fn pack_dir_inner(
    base: &Path,
    current: &Path,
    builder: &mut Builder<GzEncoder<&mut Vec<u8>>>,
) -> Result<()> {
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
        let relative = path
            .strip_prefix(base)
            .map_err(|source| SnpmError::Io {
                path: path.clone(),
                source: std::io::Error::other(source),
            })?
            .to_path_buf();

        let metadata = fs::symlink_metadata(&path).map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })?;

        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path).map_err(|source| SnpmError::ReadFile {
                path: path.clone(),
                source,
            })?;
            let mut header = Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_mode(0o777);
            builder
                .append_link(&mut header, &relative, &target)
                .map_err(|source| SnpmError::WriteFile {
                    path: path.clone(),
                    source,
                })?;
            continue;
        }

        if metadata.is_dir() {
            // The tar crate emits a directory header automatically when
            // appending files below it, but explicit headers preserve
            // permissions and empty-dir membership.
            let mut header = Header::new_gnu();
            header.set_entry_type(tar::EntryType::Directory);
            header.set_size(0);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                header.set_mode(metadata.permissions().mode());
            }
            #[cfg(not(unix))]
            {
                header.set_mode(0o755);
            }
            builder
                .append_data(&mut header, &relative, std::io::empty())
                .map_err(|source| SnpmError::WriteFile {
                    path: path.clone(),
                    source,
                })?;
            pack_dir_inner(base, &path, builder)?;
            continue;
        }

        let mut header = Header::new_gnu();
        header.set_size(metadata.len());
        header.set_entry_type(tar::EntryType::Regular);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            header.set_mode(metadata.permissions().mode());
        }
        #[cfg(not(unix))]
        {
            header.set_mode(0o644);
        }
        let mut file = fs::File::open(&path).map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })?;
        builder
            .append_data(&mut header, &relative, &mut file)
            .map_err(|source| SnpmError::WriteFile {
                path: path.clone(),
                source,
            })?;
    }

    Ok(())
}

/// Unpack a gzipped tar from `bytes` into `dest_dir`, replacing its
/// previous contents. The archive is fully decoded into a staging
/// sibling first: a corrupt payload (truncated body, proxy error page
/// served with a 200) must leave the live package directory untouched.
pub(super) fn unpack_into_dir(bytes: &[u8], dest_dir: &Path) -> Result<()> {
    let file_name = dest_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "package".to_string());
    let staging = dest_dir.with_file_name(format!(
        ".{file_name}.remote-restore.{}.tmp",
        std::process::id()
    ));
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    fs::create_dir_all(&staging).map_err(|source| SnpmError::WriteFile {
        path: staging.clone(),
        source,
    })?;

    let mut archive = Archive::new(GzDecoder::new(Cursor::new(bytes)));
    archive.set_preserve_permissions(true);
    archive.set_overwrite(true);
    if let Err(source) = archive.unpack(&staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(SnpmError::WriteFile {
            path: dest_dir.to_path_buf(),
            source,
        });
    }

    // Decode succeeded — now the swap. Losing the old dir here is fine:
    // the staged tree fully replaces it.
    if dest_dir.exists() {
        let _ = fs::remove_dir_all(dest_dir);
    }
    if let Err(source) = fs::rename(&staging, dest_dir) {
        let _ = fs::remove_dir_all(&staging);
        return Err(SnpmError::WriteFile {
            path: dest_dir.to_path_buf(),
            source,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{RemoteCache, pack_dir, unpack_into_dir};
    use crate::config::SnpmConfig;
    
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    
    use std::sync::{Arc, Mutex};
    use std::thread;
    use tempfile::tempdir;

    fn fake_config(url: Option<String>) -> SnpmConfig {
    SnpmConfig {
        registry_concurrency: 16,
        remote_cache_url: url,
        remote_cache_auth_token: Some("test-token".to_string()),
        ..SnpmConfig::for_tests()
    }
}

    #[test]
    fn from_config_returns_none_when_no_url() {
        assert!(RemoteCache::from_config(&fake_config(None)).is_none());
        assert!(RemoteCache::from_config(&fake_config(Some(String::new()))).is_none());
    }

    #[test]
    fn from_config_yields_remote_when_url_present() {
        let remote = RemoteCache::from_config(&fake_config(Some(
            "https://cache.example.com/snpm".to_string(),
        )));
        assert!(remote.is_some());
    }

    #[test]
    fn round_trip_through_fake_http_server() {
        // End-to-end: pack a source dir, PUT to a fake server, then
        // GET it back via try_restore into a fresh destination and
        // verify the bytes match.
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let restored = dir.path().join("restored");
        fs::create_dir_all(source.join("inner")).unwrap();
        fs::write(source.join("a.txt"), "first").unwrap();
        fs::write(source.join("inner/b.txt"), "second").unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}");
        let storage: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
        let received_auth: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let storage_for_server = storage.clone();
        let auth_for_server = received_auth.clone();

        let handle = thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let mut stream = stream.unwrap();
                let mut buffer = [0_u8; 8192];
                let mut headers = Vec::new();
                let mut body_start = None;
                loop {
                    let read = stream.read(&mut buffer).unwrap();
                    if read == 0 {
                        break;
                    }
                    headers.extend_from_slice(&buffer[..read]);
                    if let Some(pos) = headers.windows(4).position(|window| window == b"\r\n\r\n") {
                        body_start = Some(pos + 4);
                        break;
                    }
                }
                let headers_text = String::from_utf8_lossy(&headers).to_string();
                let is_put = headers_text.starts_with("PUT ");

                if let Some(line) = headers_text
                    .lines()
                    .find(|line| line.to_ascii_lowercase().starts_with("authorization:"))
                {
                    *auth_for_server.lock().unwrap() = Some(line.to_string());
                }

                if is_put {
                    let content_length = headers_text
                        .lines()
                        .find_map(|line| {
                            let lower = line.to_ascii_lowercase();
                            lower
                                .strip_prefix("content-length:")
                                .map(|value| value.trim().parse::<usize>().ok().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    let mut body = if let Some(start) = body_start {
                        headers[start..].to_vec()
                    } else {
                        Vec::new()
                    };
                    while body.len() < content_length {
                        let read = stream.read(&mut buffer).unwrap();
                        if read == 0 {
                            break;
                        }
                        body.extend_from_slice(&buffer[..read]);
                    }
                    *storage_for_server.lock().unwrap() = Some(body);
                    stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        )
                        .unwrap();
                } else {
                    // GET — serve the stored body or 404.
                    let body = storage_for_server.lock().unwrap().clone();
                    match body {
                        Some(body) => {
                            let header = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            );
                            stream.write_all(header.as_bytes()).unwrap();
                            stream.write_all(&body).unwrap();
                        }
                        None => {
                            stream
                                .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                                .unwrap();
                        }
                    }
                }
            }
        });

        let remote = RemoteCache::from_config(&fake_config(Some(url.clone()))).unwrap();

        // Upload step.
        remote.try_upload("pkg/key.tar.gz", &source);
        assert!(
            storage.lock().unwrap().is_some(),
            "server should have received a PUT body"
        );
        let auth = received_auth.lock().unwrap().clone();
        assert!(
            auth.is_some_and(|line| line.to_ascii_lowercase().contains("bearer test-token")),
            "PUT should send the Authorization: Bearer header"
        );

        // Restore step.
        let restored_ok = remote.try_restore("pkg/key.tar.gz", &restored);
        assert!(restored_ok, "GET should restore the body");
        assert_eq!(fs::read_to_string(restored.join("a.txt")).unwrap(), "first");
        assert_eq!(
            fs::read_to_string(restored.join("inner/b.txt")).unwrap(),
            "second"
        );

        handle.join().unwrap();
    }

    #[test]
    fn read_only_skips_upload() {
        let mut config = fake_config(Some("http://127.0.0.1:1/never-reached".to_string()));
        config.remote_cache_read_only = true;
        let remote = RemoteCache::from_config(&config).unwrap();
        let dir = tempdir().unwrap();
        // If try_upload weren't gated by read_only it would attempt
        // a TCP connect to 127.0.0.1:1 (which would fail). Since
        // failures are swallowed, the proof here is just that this
        // doesn't panic and doesn't hang.
        remote.try_upload("any-key", dir.path());
    }

    #[test]
    fn round_trip_preserves_files_and_subdirs() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let dest = dir.path().join("dest");

        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("top.txt"), "hello").unwrap();
        fs::write(source.join("nested/inner.txt"), "world").unwrap();

        let archive = pack_dir(&source).unwrap();
        assert!(!archive.is_empty(), "pack_dir produced empty bytes");
        unpack_into_dir(&archive, &dest).unwrap();

        assert_eq!(fs::read_to_string(dest.join("top.txt")).unwrap(), "hello");
        assert_eq!(
            fs::read_to_string(dest.join("nested/inner.txt")).unwrap(),
            "world"
        );
    }

    #[test]
    fn corrupt_archive_leaves_existing_destination_untouched() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("dest");
        fs::create_dir_all(&dest).unwrap();
        fs::write(dest.join("keep.txt"), "still here").unwrap();

        // Not a gzip stream — decode must fail before dest is touched.
        let result = unpack_into_dir(b"<html>captive portal</html>", &dest);
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(dest.join("keep.txt")).unwrap(),
            "still here"
        );
    }

    #[cfg(unix)]
    #[test]
    fn round_trip_preserves_executable_bit() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let dest = dir.path().join("dest");
        fs::create_dir_all(&source).unwrap();
        let script = source.join("run.sh");
        fs::write(&script, "#!/bin/sh\necho ok\n").unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();

        let archive = pack_dir(&source).unwrap();
        unpack_into_dir(&archive, &dest).unwrap();

        let mode = fs::metadata(dest.join("run.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o111,
            0o111,
            "executable bits should round-trip; got {mode:o}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn round_trip_preserves_symlinks() {
        use std::os::unix::fs::symlink;
        let dir = tempdir().unwrap();
        let source = dir.path().join("source");
        let dest = dir.path().join("dest");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("real.txt"), "hello").unwrap();
        symlink("real.txt", source.join("link.txt")).unwrap();

        let archive = pack_dir(&source).unwrap();
        unpack_into_dir(&archive, &dest).unwrap();

        let link_meta = fs::symlink_metadata(dest.join("link.txt")).unwrap();
        assert!(link_meta.file_type().is_symlink());
        let target = fs::read_link(dest.join("link.txt")).unwrap();
        assert_eq!(target.to_str(), Some("real.txt"));
    }
}
