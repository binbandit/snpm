use crate::SnpmConfig;
use crate::cache::load_cached_headers;
use crate::protocols::npm::registry::npm_like_registry_for_package;

use reqwest::Client;
use reqwest::header::{ACCEPT, HeaderValue};

pub(super) fn registry_url(config: &SnpmConfig, protocol_name: &str, name: &str) -> String {
    let encoded = super::super::super::encode_package_name(name);
    let base = npm_like_registry_for_package(config, protocol_name, name);
    format!("{}/{}", base.trim_end_matches('/'), encoded)
}

pub(super) fn build_request(
    config: &SnpmConfig,
    client: &Client,
    name: &str,
    url: &str,
) -> reqwest::RequestBuilder {
    let mut request = client.get(url);
    let accept = if config.min_package_age_days.is_some() {
        HeaderValue::from_static("application/json; q=1.0, */*")
    } else {
        HeaderValue::from_static(
            "application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8, */*",
        )
    };
    request = request.header(ACCEPT, accept);

    if let Some(header_value) = config.authorization_header_for_url(url) {
        request = request.header("authorization", header_value);
    }

    if let Some(headers) = load_cached_headers(config, name) {
        if let Some(etag) = headers.etag
            && let Ok(header_value) = HeaderValue::from_str(&etag)
        {
            request = request.header("if-none-match", header_value);
        }
        if let Some(last_modified) = headers.last_modified
            && let Ok(header_value) = HeaderValue::from_str(&last_modified)
        {
            request = request.header("if-modified-since", header_value);
        }
    }

    request
}

#[cfg(test)]
mod tests {
    use super::build_request;
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
    use reqwest::header::ACCEPT;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    fn make_config(min_package_age_days: Option<u32>) -> SnpmConfig {
        SnpmConfig {
            cache_dir: PathBuf::from("/tmp/cache"),
            data_dir: PathBuf::from("/tmp/data"),
            allow_scripts: BTreeSet::new(),
            disable_global_virtual_store_for_packages: BTreeSet::new(),
            min_package_age_days,
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

    #[test]
    fn min_package_age_requests_full_packument_metadata() {
        let client = reqwest::Client::new();
        let request = build_request(
            &make_config(Some(7)),
            &client,
            "pkg",
            "https://registry.npmjs.org/pkg",
        )
        .build()
        .unwrap();

        assert_eq!(
            request
                .headers()
                .get(ACCEPT)
                .and_then(|value| value.to_str().ok()),
            Some("application/json; q=1.0, */*")
        );
    }
}
