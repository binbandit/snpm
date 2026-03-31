use crate::{Result, SnpmConfig, SnpmError, http};

use super::PublishOptions;
use super::manifest::PackageIdentity;

pub(super) async fn send_publish_request(
    config: &SnpmConfig,
    package: &PackageIdentity,
    options: &PublishOptions,
    payload: serde_json::Value,
) -> Result<()> {
    let url = publish_url(config, &package.name);
    let client = http::create_client()?;
    let mut request = client.put(&url).json(&payload);

    if let Some(header_value) = config.authorization_header_for_url(&url) {
        request = request.header("authorization", header_value);
    }

    if let Some(otp) = &options.otp {
        request = request.header("npm-otp", otp);
    }

    let response = request.send().await.map_err(|source| SnpmError::Http {
        url: url.clone(),
        source,
    })?;

    if response.status().is_success() {
        return Ok(());
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    Err(SnpmError::PublishFailed {
        name: package.name.clone(),
        version: package.version.clone(),
        reason: format!("registry returned {} — {}", status.as_u16(), body),
    })
}

fn publish_url(config: &SnpmConfig, name: &str) -> String {
    let encoded_name = crate::protocols::encode_package_name(name);
    format!(
        "{}/{}",
        config.default_registry.trim_end_matches('/'),
        encoded_name
    )
}
