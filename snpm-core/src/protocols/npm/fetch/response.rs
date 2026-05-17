use crate::cache::{
    CachedHeaders, load_metadata_with_offline, save_metadata, save_metadata_with_headers,
};
use crate::config::OfflineMode;
use crate::console;
use crate::registry::RegistryPackage;
use crate::{Result, SnpmConfig, SnpmError};

pub(super) fn load_cached_package(
    config: &SnpmConfig,
    name: &str,
    offline_mode: OfflineMode,
) -> Option<RegistryPackage> {
    load_metadata_with_offline(config, name, offline_mode)
}

pub(super) async fn handle_registry_response(
    config: &SnpmConfig,
    name: &str,
    url: &str,
    response: reqwest::Response,
) -> Result<RegistryPackage> {
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        // 304 means "your cached version is current." If we don't actually
        // have a cached version (registry cache evicted, mismatched ETag
        // persisted, etc.), falling through to JSON-parse the empty body
        // produces a confusing "failed to parse JSON" error. Surface the
        // actual problem instead.
        return match load_metadata_with_offline(config, name, OfflineMode::Offline) {
            Some(cached) => {
                console::verbose(&format!("registry 304: using cached metadata for {}", name));
                let _ = save_metadata(config, name, &cached);
                Ok(cached)
            }
            None => Err(SnpmError::Internal {
                reason: format!(
                    "registry returned 304 Not Modified for {name} ({url}) but no cached metadata is available — delete the local metadata cache or re-run with a fresh registry to recover"
                ),
            }),
        };
    }

    let response_etag = response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(String::from);
    let response_last_modified = response
        .headers()
        .get("last-modified")
        .and_then(|value| value.to_str().ok())
        .map(String::from);

    let package = response
        .error_for_status()
        .map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?
        .json::<RegistryPackage>()
        .await
        .map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?;

    console::verbose(&format!(
        "registry decode: name={} versions={} dist_tags={}",
        name,
        package.versions.len(),
        package.dist_tags.len()
    ));

    let response_headers =
        (response_etag.is_some() || response_last_modified.is_some()).then_some(CachedHeaders {
            etag: response_etag,
            last_modified: response_last_modified,
        });

    let _ = save_metadata_with_headers(config, name, &package, response_headers.as_ref());

    Ok(package)
}
