use crate::console;
use crate::project::Manifest;
use crate::{Result, SnpmConfig, SnpmError};
use reqwest::Client;
use reqwest::header::{ACCEPT, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::process::Command;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegistryPackage {
    pub versions: BTreeMap<String, RegistryVersion>,
    #[serde(default)]
    pub time: BTreeMap<String, serde_json::Value>,
    #[serde(default, rename = "dist-tags")]
    pub dist_tags: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PeerDependencyMeta {
    #[serde(default)]
    pub optional: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BundledDependencies {
    List(Vec<String>),
    All(bool),
}

impl BundledDependencies {
    pub fn to_set(
        &self,
        all_deps: &BTreeMap<String, String>,
    ) -> std::collections::BTreeSet<String> {
        match self {
            BundledDependencies::List(list) => list.iter().cloned().collect(),
            BundledDependencies::All(true) => all_deps.keys().cloned().collect(),
            BundledDependencies::All(false) => std::collections::BTreeSet::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            BundledDependencies::List(list) => list.is_empty(),
            BundledDependencies::All(val) => !val,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegistryVersion {
    pub version: String,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    #[serde(default, rename = "optionalDependencies")]
    pub optional_dependencies: BTreeMap<String, String>,
    #[serde(default, rename = "peerDependencies")]
    pub peer_dependencies: BTreeMap<String, String>,
    #[serde(default, rename = "peerDependenciesMeta")]
    pub peer_dependencies_meta: BTreeMap<String, PeerDependencyMeta>,
    #[serde(default, rename = "bundledDependencies")]
    pub bundled_dependencies: Option<BundledDependencies>,
    #[serde(default, rename = "bundleDependencies")]
    pub bundle_dependencies: Option<BundledDependencies>,
    pub dist: RegistryDist,
    #[serde(default)]
    pub os: Vec<String>,
    #[serde(default)]
    pub cpu: Vec<String>,
    #[serde(default)]
    pub bin: Option<serde_json::Value>,
}

impl RegistryVersion {
    pub fn get_bundled_dependencies(&self) -> Option<&BundledDependencies> {
        self.bundled_dependencies
            .as_ref()
            .or(self.bundle_dependencies.as_ref())
    }

    pub fn has_bin(&self) -> bool {
        self.bin.as_ref().map(|b| !b.is_null()).unwrap_or(false)
    }
}

#[derive(Clone, Debug)]
pub struct RegistryProtocol {
    pub name: String,
}

impl RegistryProtocol {
    pub fn npm() -> Self {
        RegistryProtocol {
            name: "npm".to_string(),
        }
    }

    pub fn jsr() -> Self {
        RegistryProtocol {
            name: "jsr".to_string(),
        }
    }

    pub fn custom(name: &str) -> Self {
        RegistryProtocol {
            name: name.to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegistryDist {
    pub tarball: String,
    #[serde(default)]
    pub integrity: Option<String>,
}

fn sanitize_package_name(name: &str) -> String {
    name.replace('/', "__")
}

fn load_cached_metadata(config: &SnpmConfig, name: &str) -> Option<RegistryPackage> {
    let sanitized = sanitize_package_name(name);
    let cache_path = config.metadata_dir().join(&sanitized).join("index.json");

    if !cache_path.exists() {
        return None;
    }

    if let Ok(data) = fs::read_to_string(&cache_path) {
        if let Ok(package) = serde_json::from_str::<RegistryPackage>(&data) {
            if is_cache_fresh(config, &cache_path) {
                if console::is_logging_enabled() {
                    console::verbose(&format!(
                        "using cached metadata for {} from {}",
                        name,
                        cache_path.display()
                    ));
                }
                return Some(package);
            } else if console::is_logging_enabled() {
                console::verbose(&format!(
                    "cached metadata for {} is stale, will refetch",
                    name
                ));
            }
        }
    }

    None
}

fn save_cached_metadata(config: &SnpmConfig, name: &str, package: &RegistryPackage) -> Result<()> {
    let sanitized = sanitize_package_name(name);
    let cache_dir = config.metadata_dir().join(&sanitized);
    let cache_path = cache_dir.join("index.json");

    if let Err(e) = fs::create_dir_all(&cache_dir) {
        if console::is_logging_enabled() {
            console::verbose(&format!(
                "failed to create metadata cache dir {}: {}",
                cache_dir.display(),
                e
            ));
        }
        return Ok(());
    }

    match serde_json::to_string_pretty(package) {
        Ok(json) => {
            if let Err(e) = fs::write(&cache_path, json) {
                if console::is_logging_enabled() {
                    console::verbose(&format!(
                        "failed to write metadata cache for {}: {}",
                        name, e
                    ));
                }
            } else if console::is_logging_enabled() {
                console::verbose(&format!(
                    "saved metadata cache for {} to {}",
                    name,
                    cache_path.display()
                ));
            }
        }
        Err(e) => {
            if console::is_logging_enabled() {
                console::verbose(&format!("failed to serialize metadata for {}: {}", name, e));
            }
        }
    }

    Ok(())
}

fn is_cache_fresh(config: &SnpmConfig, cache_path: &Path) -> bool {
    let Some(max_age_days) = config.min_package_cache_age_days else {
        return false;
    };

    if let Ok(metadata) = fs::metadata(cache_path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                let age_days = elapsed.as_secs() / 86400;
                return age_days < max_age_days as u64;
            }
        }
    }

    false
}

pub async fn fetch_package(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
    protocol: &RegistryProtocol,
) -> Result<RegistryPackage> {
    if protocol.name == "jsr" {
        fetch_jsr_package(config, client, name).await
    } else if protocol.name == "file" {
        fetch_file_package(config, name).await
    } else if protocol.name == "git" {
        fetch_git_package(config, name).await
    } else {
        fetch_npm_like_package(config, client, name, &protocol.name).await
    }
}

async fn fetch_file_package(_config: &SnpmConfig, path_str: &str) -> Result<RegistryPackage> {
    let cwd = env::current_dir().map_err(|e| SnpmError::Io {
        path: PathBuf::from("."),
        source: e,
    })?;

    let path = Path::new(path_str);
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };

    if !abs_path.exists() {
        return Err(SnpmError::ResolutionFailed {
            name: path_str.to_string(),
            range: "latest".to_string(),
            reason: format!("File path does not exist: {}", abs_path.display()),
        });
    }

    if abs_path.is_dir() {
        let manifest_path = abs_path.join("package.json");
        let manifest_content =
            fs::read_to_string(&manifest_path).map_err(|e| SnpmError::ReadFile {
                path: manifest_path.clone(),
                source: e,
            })?;

        let manifest: Manifest =
            serde_json::from_str(&manifest_content).map_err(|e| SnpmError::ParseJson {
                path: manifest_path.clone(),
                source: e,
            })?;

        let version = manifest
            .version
            .clone()
            .unwrap_or_else(|| "0.0.0".to_string());

        let reg_version =
            registry_version_from_manifest(manifest, &format!("file://{}", abs_path.display()));

        let mut versions = BTreeMap::new();
        versions.insert(version.clone(), reg_version);

        let mut dist_tags = BTreeMap::new();
        dist_tags.insert("latest".to_string(), version);

        Ok(RegistryPackage {
            versions,
            time: BTreeMap::new(),
            dist_tags,
        })
    } else {
        Err(SnpmError::ResolutionFailed {
            name: path_str.to_string(),
            range: "latest".to_string(),
            reason: "Single file dependencies are not yet fully supported (expected directory)"
                .to_string(),
        })
    }
}

async fn fetch_git_package(config: &SnpmConfig, url: &str) -> Result<RegistryPackage> {
    let safe_name = url.replace(|c: char| !c.is_alphanumeric(), "_");
    let cache_dir = config.cache_dir.join("git").join(&safe_name);

    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir).map_err(|e| SnpmError::Io {
            path: cache_dir.clone(),
            source: e,
        })?;
    }

    let repo_dir = cache_dir.join("repo");

    if repo_dir.exists() {
        let status = Command::new("git")
            .current_dir(&repo_dir)
            .arg("fetch")
            .arg("--all")
            .status()
            .await
            .map_err(|e| SnpmError::ResolutionFailed {
                name: url.to_string(),
                range: "latest".to_string(),
                reason: format!("Failed to run git fetch: {}", e),
            })?;

        if !status.success() {
            return Err(SnpmError::ResolutionFailed {
                name: url.to_string(),
                range: "latest".to_string(),
                reason: "git fetch failed".to_string(),
            });
        }
    } else {
        let mut clean_url = url.to_string();
        let mut committish = None;

        if let Some(idx) = url.rfind('#') {
            committish = Some(&url[idx + 1..]);
            clean_url = url[..idx].to_string();
        }

        let status = Command::new("git")
            .current_dir(&cache_dir)
            .arg("clone")
            .arg(&clean_url)
            .arg("repo")
            .status()
            .await
            .map_err(|e| SnpmError::ResolutionFailed {
                name: url.to_string(),
                range: "latest".to_string(),
                reason: format!("Failed to run git clone: {}", e),
            })?;

        if !status.success() {
            return Err(SnpmError::ResolutionFailed {
                name: url.to_string(),
                range: "latest".to_string(),
                reason: "git clone failed".to_string(),
            });
        }

        if let Some(rev) = committish {
            let status = Command::new("git")
                .current_dir(&repo_dir)
                .arg("checkout")
                .arg(rev)
                .status()
                .await
                .map_err(|e| SnpmError::ResolutionFailed {
                    name: url.to_string(),
                    range: rev.to_string(),
                    reason: format!("Failed to run git checkout: {}", e),
                })?;

            if !status.success() {
                return Err(SnpmError::ResolutionFailed {
                    name: url.to_string(),
                    range: rev.to_string(),
                    reason: "git checkout failed".to_string(),
                });
            }
        }
    }

    let manifest_path = repo_dir.join("package.json");
    if !manifest_path.exists() {
        return Err(SnpmError::ResolutionFailed {
            name: url.to_string(),
            range: "latest".to_string(),
            reason: "package.json not found in git repo".to_string(),
        });
    }

    let manifest_content = fs::read_to_string(&manifest_path).map_err(|e| SnpmError::ReadFile {
        path: manifest_path.clone(),
        source: e,
    })?;

    let manifest: Manifest =
        serde_json::from_str(&manifest_content).map_err(|e| SnpmError::ParseJson {
            path: manifest_path.clone(),
            source: e,
        })?;

    let version = manifest
        .version
        .clone()
        .unwrap_or_else(|| "0.0.0".to_string());

    let reg_version =
        registry_version_from_manifest(manifest, &format!("file://{}", repo_dir.display()));

    let mut versions = BTreeMap::new();
    versions.insert(version.clone(), reg_version);

    let mut dist_tags = BTreeMap::new();
    dist_tags.insert("latest".to_string(), version);

    Ok(RegistryPackage {
        versions,
        time: BTreeMap::new(),
        dist_tags,
    })
}

fn registry_version_from_manifest(manifest: Manifest, dist_url: &str) -> RegistryVersion {
    RegistryVersion {
        version: manifest.version.unwrap_or_else(|| "0.0.0".to_string()),
        dependencies: manifest.dependencies,
        optional_dependencies: BTreeMap::new(),
        peer_dependencies: BTreeMap::new(),
        peer_dependencies_meta: BTreeMap::new(),
        bundled_dependencies: None,
        bundle_dependencies: None,
        dist: RegistryDist {
            tarball: dist_url.to_string(),
            integrity: None,
        },
        os: vec![],
        cpu: vec![],
        bin: None,
    }
}

fn encode_package_name(name: &str) -> String {
    if name.starts_with('@') {
        name.replace('/', "%2F")
    } else {
        name.to_string()
    }
}

async fn fetch_npm_like_package(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
    protocol_name: &str,
) -> Result<RegistryPackage> {
    if let Some(cached) = load_cached_metadata(config, name) {
        return Ok(cached);
    }

    let encoded = encode_package_name(name);
    let base = npm_like_registry_for_package(config, protocol_name, name);
    let url = format!("{}/{}", base.trim_end_matches('/'), encoded);

    let mut request = client.get(&url);

    request = request.header(
        ACCEPT,
        HeaderValue::from_static(
            "application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8, */*",
        ),
    );

    let mut auth_token = config.auth_token_for_url(&url).map(|t| t.to_string());

    if auth_token.is_none() && config.always_auth {
        if let Some(default_host) = crate::config::host_from_url(&config.default_registry) {
            if let Some(req_host) = crate::config::host_from_url(&url) {
                if req_host == default_host {
                    if let Some(def_tok) = config.default_registry_auth_token.as_ref() {
                        auth_token = Some(def_tok.clone());
                    }
                }
            }
        }
    }

    if let Some(token) = auth_token {
        // Decide scheme: default registry uses configured scheme; others default to Bearer
        let mut use_basic = false;
        if let Some(default_host) = crate::config::host_from_url(&config.default_registry) {
            if let Some(req_host) = crate::config::host_from_url(&url) {
                if req_host == default_host {
                    use_basic = matches!(
                        config.default_registry_auth_scheme,
                        crate::config::AuthScheme::Basic
                    );
                }
            }
        }

        let header_value = if use_basic {
            format!("Basic {}", token)
        } else {
            format!("Bearer {}", token)
        };
        request = request.header("authorization", header_value);
    }

    if console::is_logging_enabled() {
        console::verbose(&format!(
            "registry request: name={} protocol={} url={}",
            name, protocol_name, url
        ));
    }
    let started = Instant::now();

    let response = request.send().await.map_err(|source| SnpmError::Http {
        url: url.clone(),
        source,
    })?;

    let status = response.status();
    if console::is_logging_enabled() {
        console::verbose(&format!(
            "registry response: name={} status={} in {:.3}s",
            name,
            status.as_u16(),
            started.elapsed().as_secs_f64()
        ));
    }

    let package = response
        .error_for_status()
        .map_err(|source| SnpmError::Http {
            url: url.clone(),
            source,
        })?
        .json::<RegistryPackage>()
        .await
        .map_err(|source| SnpmError::Http {
            url: url.clone(),
            source,
        })?;

    if console::is_logging_enabled() {
        console::verbose(&format!(
            "registry decode: name={} versions={} dist_tags={}",
            name,
            package.versions.len(),
            package.dist_tags.len()
        ));
    }

    let _ = save_cached_metadata(config, name, &package);

    Ok(package)
}

fn npm_like_registry_for_package(config: &SnpmConfig, protocol_name: &str, name: &str) -> String {
    if protocol_name == "npm" {
        return npm_registry_for_package(config, name);
    }

    let key = format!("SNPM_REGISTRY_{}", protocol_name.to_uppercase());
    if let Ok(value) = env::var(&key) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_string();
        }
    }

    config.default_registry.clone()
}

fn npm_registry_for_package(config: &SnpmConfig, name: &str) -> String {
    if let Some((scope, _)) = name.split_once('/') {
        if scope.starts_with('@') {
            if let Some(reg) = config.scoped_registries.get(scope) {
                return reg.clone();
            }
        }
    }

    config.default_registry.clone()
}

async fn fetch_jsr_package(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
) -> Result<RegistryPackage> {
    let compat = jsr_compat_name(name);

    if let Some(cached) = load_cached_metadata(config, &compat) {
        return Ok(cached);
    }

    let encoded = encode_package_name(&compat);
    let base = jsr_registry_base();
    let url = format!("{}/{}", base.trim_end_matches('/'), encoded);

    let mut request = client.get(&url);

    request = request.header(
        ACCEPT,
        HeaderValue::from_static(
            "application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8, */*",
        ),
    );

    let mut auth_token = config.auth_token_for_url(&url).map(|t| t.to_string());

    if auth_token.is_none() && config.always_auth {
        if let Some(default_host) = crate::config::host_from_url(&config.default_registry) {
            if let Some(req_host) = crate::config::host_from_url(&url) {
                if req_host == default_host {
                    if let Some(def_tok) = config.default_registry_auth_token.as_ref() {
                        auth_token = Some(def_tok.clone());
                    }
                }
            }
        }
    }

    if let Some(token) = auth_token {
        let mut use_basic = false;
        if let Some(default_host) = crate::config::host_from_url(&config.default_registry) {
            if let Some(req_host) = crate::config::host_from_url(&url) {
                if req_host == default_host {
                    use_basic = matches!(
                        config.default_registry_auth_scheme,
                        crate::config::AuthScheme::Basic
                    );
                }
            }
        }

        let header_value = if use_basic {
            format!("Basic {}", token)
        } else {
            format!("Bearer {}", token)
        };
        request = request.header("authorization", header_value);
    }

    if console::is_logging_enabled() {
        console::verbose(&format!(
            "registry request (jsr): name={} compat={} url={}",
            name, compat, url
        ));
    }
    let started = Instant::now();

    let response = request.send().await.map_err(|source| SnpmError::Http {
        url: url.clone(),
        source,
    })?;

    let status = response.status();
    if console::is_logging_enabled() {
        console::verbose(&format!(
            "registry response (jsr): name={} status={} in {:.3}s",
            name,
            status.as_u16(),
            started.elapsed().as_secs_f64()
        ));
    }

    let package = response
        .error_for_status()
        .map_err(|source| SnpmError::Http {
            url: url.clone(),
            source,
        })?
        .json::<RegistryPackage>()
        .await
        .map_err(|source| SnpmError::Http {
            url: url.clone(),
            source,
        })?;

    if console::is_logging_enabled() {
        console::verbose(&format!(
            "registry decode (jsr): name={} versions={} dist_tags={}",
            name,
            package.versions.len(),
            package.dist_tags.len()
        ));
    }

    let _ = save_cached_metadata(config, &compat, &package);

    Ok(package)
}

fn jsr_registry_base() -> String {
    if let Ok(value) = env::var("SNPM_REGISTRY_JSR") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_string();
        }
    }

    "https://npm.jsr.io".to_string()
}

fn jsr_compat_name(name: &str) -> String {
    if name.starts_with("@jsr/") {
        return name.to_string();
    }

    if let Some(stripped) = name.strip_prefix('@') {
        if let Some((scope, pkg)) = stripped.split_once('/') {
            return format!("@jsr/{}__{}", scope, pkg);
        } else {
            return format!("@jsr/{}", stripped);
        }
    }

    if let Some((scope, pkg)) = name.split_once('/') {
        return format!("@jsr/{}__{}", scope, pkg);
    }

    format!("@jsr/{}", name)
}
