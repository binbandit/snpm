use crate::{Result, SnpmConfig, SnpmError};
use futures::StreamExt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tempfile::{Builder, TempPath};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use super::archive::{unpack_tarball_file, unpack_tarball_reader};
use super::integrity::{IntegrityCacheKey, IntegritySpec};
use super::limits::download_semaphore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TarballSource {
    Downloaded,
    BlobCache,
}

#[derive(Debug)]
pub(super) struct DownloadedTarball {
    path: PathBuf,
    size_bytes: u64,
    source: TarballSource,
    _temp_path: Option<TempPath>,
}

impl DownloadedTarball {
    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub(super) fn source(&self) -> TarballSource {
        self.source
    }
}

/// Materialize a tarball into `target_dir`, downloading and extracting concurrently
/// on the cache-miss path. On cache hit, falls back to reading the cached blob
/// from disk and extracting it. `target_dir` must already be prepared (empty).
pub(super) async fn download_and_extract(
    config: &SnpmConfig,
    package_name: &str,
    url: &str,
    integrity: Option<&str>,
    client: &reqwest::Client,
    target_dir: &Path,
) -> Result<DownloadedTarball> {
    let integrity_spec = IntegritySpec::parse(integrity);
    let blob_cache_paths = cached_blob_paths(config, url, integrity_spec.as_ref())?;

    if let Some(cached_path) = blob_cache_paths.iter().find(|path| path.is_file()) {
        let size_bytes = fs::metadata(cached_path)
            .map_err(|source| SnpmError::ReadFile {
                path: cached_path.clone(),
                source,
            })?
            .len();
        let cached = cached_path.clone();
        let target = target_dir.to_path_buf();
        tokio::task::spawn_blocking(move || unpack_tarball_file(&target, &cached))
            .await
            .map_err(|error| SnpmError::StoreTask {
                reason: error.to_string(),
            })??;
        return Ok(DownloadedTarball {
            path: cached_path.clone(),
            size_bytes,
            source: TarballSource::BlobCache,
            _temp_path: None,
        });
    }

    let _download_permit =
        download_semaphore()
            .acquire()
            .await
            .map_err(|error| SnpmError::Internal {
                reason: format!("download semaphore closed while fetching {url}: {error}"),
            })?;

    let temp_parent = config.tarball_blob_cache_dir().join("tmp");
    let temp_path = create_temp_path(&temp_parent)?;
    let file_path = temp_path.to_path_buf();
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&file_path)
        .await
        .map_err(|source| SnpmError::WriteFile {
            path: file_path.clone(),
            source,
        })?;

    let mut verifier = integrity_spec.as_ref().map(IntegritySpec::verifier);
    let mut request = client.get(url);
    if let Some(header_value) = config.authorization_header_for_tarball(package_name, url) {
        request = request.header("authorization", header_value);
    }

    let response = request
        .send()
        .await
        .map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?
        .error_for_status()
        .map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?;

    // Pre-size the blob cache temp file from Content-Length when the server
    // advertises one. Reduces filesystem fragmentation on cold writes and lets
    // the kernel pick a single extent for the .tgz on most filesystems.
    if let Some(content_length) = response.content_length()
        && content_length > 0
    {
        let _ = file.set_len(content_length).await;
    }

    // Bounded channel so the network task applies backpressure when the
    // extractor falls behind (and vice versa). 8 chunks ≈ ~64KB–1MB of in-flight
    // buffer depending on chunk size — small enough to keep memory bounded,
    // large enough to absorb network jitter.
    let (tx, rx) = mpsc::channel::<Vec<u8>>(8);
    let target = target_dir.to_path_buf();
    let extract_task = tokio::task::spawn_blocking(move || -> Result<()> {
        let reader = ChannelReader::new(rx);
        unpack_tarball_reader(&target, reader)
    });

    let mut body_stream = response.bytes_stream();
    let mut size_bytes = 0_u64;
    let mut stream_failed: Option<SnpmError> = None;
    while let Some(chunk_result) = body_stream.next().await {
        let chunk = match chunk_result {
            Ok(chunk) => chunk,
            Err(source) => {
                stream_failed = Some(SnpmError::Http {
                    url: url.to_string(),
                    source,
                });
                break;
            }
        };
        if let Some(verifier) = verifier.as_mut() {
            verifier.update(&chunk);
        }
        file.write_all(&chunk)
            .await
            .map_err(|source| SnpmError::WriteFile {
                path: file_path.clone(),
                source,
            })?;
        size_bytes += chunk.len() as u64;
        if tx.send(chunk.to_vec()).await.is_err() {
            // Extractor dropped its receiver, almost certainly because it
            // returned an error. Stop pulling from the network and surface the
            // extractor's error below.
            break;
        }
    }
    drop(tx);

    file.flush().await.map_err(|source| SnpmError::WriteFile {
        path: file_path.clone(),
        source,
    })?;
    drop(file);

    let extract_outcome = extract_task.await.map_err(|error| SnpmError::StoreTask {
        reason: error.to_string(),
    })?;
    extract_outcome?;
    if let Some(error) = stream_failed {
        return Err(error);
    }

    let matched_cache_key = verifier
        .map(|verifier| verifier.finish(url))
        .transpose()?
        .flatten();

    if let Some(cached_path) = matched_cache_key
        .as_ref()
        .map(|cache_key| blob_cache_path(config, cache_key))
    {
        let parent = cached_path.parent().ok_or_else(|| SnpmError::Internal {
            reason: format!(
                "tarball blob cache path has no parent: {}",
                cached_path.display()
            ),
        })?;
        fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;

        match fs::rename(&file_path, &cached_path) {
            Ok(()) => {
                return Ok(DownloadedTarball {
                    path: cached_path,
                    size_bytes,
                    source: TarballSource::Downloaded,
                    _temp_path: None,
                });
            }
            Err(_source) if cached_path.is_file() => {
                let size_bytes = fs::metadata(&cached_path)
                    .map_err(|metadata_source| SnpmError::ReadFile {
                        path: cached_path.clone(),
                        source: metadata_source,
                    })?
                    .len();
                return Ok(DownloadedTarball {
                    path: cached_path,
                    size_bytes,
                    source: TarballSource::BlobCache,
                    _temp_path: Some(temp_path),
                });
            }
            Err(source) => {
                return Err(SnpmError::WriteFile {
                    path: cached_path,
                    source,
                });
            }
        }
    }

    Ok(DownloadedTarball {
        path: file_path,
        size_bytes,
        source: TarballSource::Downloaded,
        _temp_path: Some(temp_path),
    })
}

struct ChannelReader {
    rx: mpsc::Receiver<Vec<u8>>,
    current: Vec<u8>,
    offset: usize,
}

impl ChannelReader {
    fn new(rx: mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            rx,
            current: Vec::new(),
            offset: 0,
        }
    }
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        while self.offset >= self.current.len() {
            match self.rx.blocking_recv() {
                Some(chunk) => {
                    self.current = chunk;
                    self.offset = 0;
                }
                None => return Ok(0),
            }
        }
        let available = &self.current[self.offset..];
        let take = available.len().min(buf.len());
        buf[..take].copy_from_slice(&available[..take]);
        self.offset += take;
        Ok(take)
    }
}

fn cached_blob_paths(
    config: &SnpmConfig,
    url: &str,
    integrity: Option<&IntegritySpec>,
) -> Result<Vec<PathBuf>> {
    let Some(integrity) = integrity else {
        return Ok(Vec::new());
    };

    let cache_keys = integrity.cache_keys(url)?;
    Ok(cache_keys
        .into_iter()
        .map(|cache_key| blob_cache_path(config, &cache_key))
        .collect())
}

fn blob_cache_path(config: &SnpmConfig, cache_key: &IntegrityCacheKey) -> PathBuf {
    config
        .tarball_blob_cache_dir()
        .join(cache_key.algorithm())
        .join(format!("{}.tgz", cache_key.digest_hex()))
}

fn create_temp_path(parent: &Path) -> Result<TempPath> {
    fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
        path: parent.to_path_buf(),
        source,
    })?;

    Builder::new()
        .prefix(".snpm-tarball-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map(tempfile::NamedTempFile::into_temp_path)
        .map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::{TarballSource, download_and_extract};
    use crate::SnpmError;
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
    use crate::store::limits::{download_concurrency, download_semaphore};

    use base64::Engine;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use sha2::{Digest, Sha512};
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tar::{Builder as TarBuilder, EntryType, Header};
    use tempfile::tempdir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::time::{Duration, sleep, timeout};

    fn make_config(root: PathBuf) -> SnpmConfig {
        SnpmConfig {
            cache_dir: root.join("cache"),
            data_dir: root.join("data"),
            allow_scripts: BTreeSet::new(),
            disable_global_virtual_store_for_packages: BTreeSet::new(),
            min_package_age_days: None,
            min_package_cache_age_days: None,
            default_registry: "https://registry.npmjs.org".to_string(),
            scoped_registries: BTreeMap::new(),
            registry_auth: BTreeMap::new(),
            default_registry_auth_token: None,
            default_registry_auth_scheme: AuthScheme::Bearer,
            registry_auth_schemes: BTreeMap::new(),
            hoisting: HoistingMode::SingleVersion,
            link_backend: LinkBackend::Auto,
            strict_peers: false,
            frozen_lockfile_default: false,
            always_auth: false,
            registry_concurrency: 64,
            verbose: false,
            log_file: None,
        }
    }

    fn build_tarball() -> Vec<u8> {
        let mut builder = TarBuilder::new(Vec::new());
        let content = b"{\"name\":\"pkg\",\"version\":\"1.0.0\"}";
        let mut header = Header::new_gnu();
        header.set_entry_type(EntryType::Regular);
        header.set_path("package/package.json").unwrap();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, &content[..]).unwrap();

        let tar_bytes = builder.into_inner().unwrap();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&tar_bytes).unwrap();
        encoder.finish().unwrap()
    }

    async fn spawn_tarball_server(
        body: Vec<u8>,
        request_count: Arc<AtomicUsize>,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let (url, _captured, handle) = spawn_capturing_tarball_server(body, request_count).await;
        (url, handle)
    }

    async fn spawn_capturing_tarball_server(
        body: Vec<u8>,
        request_count: Arc<AtomicUsize>,
    ) -> (
        String,
        Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/pkg.tgz");
        let captured: Arc<std::sync::Mutex<Vec<Vec<u8>>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured_for_task = captured.clone();

        let handle = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                request_count.fetch_add(1, Ordering::SeqCst);

                let mut buffer = [0_u8; 1024];
                let mut request = Vec::new();
                loop {
                    let read = socket.read(&mut buffer).await.unwrap();
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }

                captured_for_task.lock().unwrap().push(request);

                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
                socket.write_all(&body).await.unwrap();
            }
        });

        (url, captured, handle)
    }

    fn request_has_authorization_header(request: &[u8]) -> bool {
        let text = std::str::from_utf8(request).unwrap_or_default();
        text.lines()
            .any(|line| line.to_ascii_lowercase().starts_with("authorization:"))
    }

    fn prepare_target_dir(root: &std::path::Path) -> PathBuf {
        let target = root.join("target");
        std::fs::create_dir_all(&target).unwrap();
        target
    }

    #[tokio::test]
    async fn download_and_reuse_verified_blob_cache() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().to_path_buf());
        let tarball = build_tarball();
        let integrity = format!(
            "sha512-{}",
            base64::engine::general_purpose::STANDARD.encode(Sha512::digest(&tarball))
        );
        let request_count = Arc::new(AtomicUsize::new(0));
        let (url, server) = spawn_tarball_server(tarball.clone(), request_count.clone()).await;
        let client = reqwest::Client::new();

        let target_first = prepare_target_dir(dir.path());
        let first = download_and_extract(
            &config,
            "pkg",
            &url,
            Some(&integrity),
            &client,
            &target_first,
        )
        .await
        .unwrap();
        let expected_path = config
            .tarball_blob_cache_dir()
            .join("sha512")
            .join(format!("{}.tgz", hex::encode(Sha512::digest(&tarball))));

        assert_eq!(first.source(), TarballSource::Downloaded);
        assert_eq!(first.path(), expected_path.as_path());
        assert_eq!(first.size_bytes(), tarball.len() as u64);
        assert!(expected_path.is_file());
        assert!(target_first.join("package/package.json").is_file());

        let target_second = dir.path().join("target-second");
        std::fs::create_dir_all(&target_second).unwrap();
        let second = download_and_extract(
            &config,
            "pkg",
            &url,
            Some(&integrity),
            &client,
            &target_second,
        )
        .await
        .unwrap();
        assert_eq!(second.source(), TarballSource::BlobCache);
        assert_eq!(second.path(), expected_path.as_path());
        assert_eq!(request_count.load(Ordering::SeqCst), 1);
        assert!(target_second.join("package/package.json").is_file());

        server.abort();
    }

    #[tokio::test]
    async fn streaming_extract_matches_unpacked_tree_on_cache_miss() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().to_path_buf());
        let tarball = build_tarball();
        let request_count = Arc::new(AtomicUsize::new(0));
        let (url, server) = spawn_tarball_server(tarball.clone(), request_count.clone()).await;
        let client = reqwest::Client::new();

        let target = prepare_target_dir(dir.path());
        let result = download_and_extract(&config, "pkg", &url, None, &client, &target)
            .await
            .unwrap();

        assert_eq!(result.source(), TarballSource::Downloaded);
        assert_eq!(result.size_bytes(), tarball.len() as u64);

        let extracted = std::fs::read_to_string(target.join("package/package.json")).unwrap();
        assert_eq!(extracted, "{\"name\":\"pkg\",\"version\":\"1.0.0\"}");

        server.abort();
    }

    #[tokio::test]
    async fn auth_header_not_sent_when_tarball_host_differs_from_registry() {
        let dir = tempdir().unwrap();
        let mut config = make_config(dir.path().to_path_buf());
        // Registry stays on its real host; tarball server is on 127.0.0.1.
        config.default_registry = "https://registry.example.invalid".to_string();
        config.default_registry_auth_token = Some("leaked-token".to_string());

        let tarball = build_tarball();
        let request_count = Arc::new(AtomicUsize::new(0));
        let (url, captured, server) = spawn_capturing_tarball_server(tarball, request_count).await;
        let client = reqwest::Client::new();

        let target = prepare_target_dir(dir.path());
        let _result = download_and_extract(&config, "pkg", &url, None, &client, &target)
            .await
            .unwrap();

        let captured = captured.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert!(
            !request_has_authorization_header(&captured[0]),
            "tarball request to {url} (different host than registry) must not include Authorization header"
        );

        server.abort();
    }

    #[tokio::test]
    async fn auth_header_sent_when_tarball_host_matches_registry() {
        let dir = tempdir().unwrap();
        let tarball = build_tarball();
        let request_count = Arc::new(AtomicUsize::new(0));
        let (url, captured, server) = spawn_capturing_tarball_server(tarball, request_count).await;

        let host = url
            .strip_prefix("http://")
            .and_then(|rest| rest.split('/').next())
            .unwrap()
            .to_string();
        let mut config = make_config(dir.path().to_path_buf());
        // Point the default registry at the same host as the tarball server so
        // the origin-match check passes and the token is sent.
        config.default_registry = format!("http://{host}");
        config.default_registry_auth_token = Some("legit-token".to_string());

        let client = reqwest::Client::new();
        let target = prepare_target_dir(dir.path());
        let _result = download_and_extract(&config, "pkg", &url, None, &client, &target)
            .await
            .unwrap();

        let captured = captured.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert!(
            request_has_authorization_header(&captured[0]),
            "tarball request on matching host must include Authorization header"
        );

        server.abort();
    }

    #[tokio::test]
    async fn invalid_integrity_does_not_leave_cached_blob() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().to_path_buf());
        let tarball = build_tarball();
        let request_count = Arc::new(AtomicUsize::new(0));
        let (url, server) = spawn_tarball_server(tarball, request_count).await;
        let client = reqwest::Client::new();

        let target = prepare_target_dir(dir.path());
        let error = download_and_extract(
            &config,
            "pkg",
            &url,
            Some("sha512-invalid"),
            &client,
            &target,
        )
        .await
        .unwrap_err();

        assert!(matches!(error, SnpmError::Tarball { .. }));
        assert!(!config.tarball_blob_cache_dir().join("sha512").exists());

        server.abort();
    }

    #[tokio::test]
    async fn download_waits_for_permit_before_creating_temp_file() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().to_path_buf());
        let permits = download_semaphore()
            .acquire_many(download_concurrency() as u32)
            .await
            .unwrap();
        let client = reqwest::Client::new();
        let target = prepare_target_dir(dir.path());
        let download = download_and_extract(
            &config,
            "pkg",
            "http://127.0.0.1:9/pkg.tgz",
            None,
            &client,
            &target,
        );
        tokio::pin!(download);

        tokio::select! {
            result = &mut download => panic!("download should wait for a permit: {result:?}"),
            _ = sleep(Duration::from_millis(25)) => {}
        }

        assert!(!config.tarball_blob_cache_dir().join("tmp").exists());

        drop(permits);
        let _ = timeout(Duration::from_secs(1), download).await;
    }
}
