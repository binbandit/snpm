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
    request = request.header(
        ACCEPT,
        HeaderValue::from_static(
            "application/vnd.npm.install-v1+json; q=1.0, application/json; q=0.8, */*",
        ),
    );

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
