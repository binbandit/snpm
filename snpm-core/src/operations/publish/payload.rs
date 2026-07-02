use crate::{Result, SnpmError};

use base64::Engine;
use sha1::Digest as _;
use std::path::Path;

use super::PublishOptions;
use super::manifest::PackageIdentity;

pub(super) fn build_publish_payload(
    registry: &str,
    package: &PackageIdentity,
    manifest_value: &serde_json::Value,
    tarball_path: &Path,
    options: &PublishOptions,
) -> Result<serde_json::Value> {
    let tarball_bytes = read_tarball_bytes(tarball_path)?;
    let dist = build_dist(registry, package, &tarball_bytes);
    let attachment_name = format!("{}-{}.tgz", tarball_basename(&package.name), package.version);
    let version_meta = build_version_meta(package, manifest_value, &dist);

    let mut payload = serde_json::json!({
        "_id": package.name.as_str(),
        "name": package.name.as_str(),
        "description": manifest_value.get("description").unwrap_or(&serde_json::Value::Null),
        "dist-tags": {
            options.tag.as_str(): package.version.as_str()
        },
        "versions": {
            package.version.as_str(): version_meta
        },
        "_attachments": {
            attachment_name: {
                "content_type": "application/octet-stream",
                "data": base64::engine::general_purpose::STANDARD.encode(&tarball_bytes),
                "length": tarball_bytes.len(),
            }
        }
    });

    // Only send access when the user asked for one: npm's default for
    // scoped packages is restricted/registry-decided, so hardcoding
    // "public" would silently expose scoped packages.
    if let Some(access) = options.access.as_deref() {
        payload["access"] = serde_json::Value::String(access.to_string());
    }

    Ok(payload)
}

/// npm tarball filenames use the name without its scope:
/// `@scope/pkg` publishes `pkg-<version>.tgz`.
fn tarball_basename(name: &str) -> &str {
    name.rsplit_once('/').map(|(_, base)| base).unwrap_or(name)
}

fn read_tarball_bytes(tarball_path: &Path) -> Result<Vec<u8>> {
    std::fs::read(tarball_path).map_err(|source| SnpmError::ReadFile {
        path: tarball_path.to_path_buf(),
        source,
    })
}

fn build_version_meta(
    package: &PackageIdentity,
    manifest_value: &serde_json::Value,
    dist: &serde_json::Value,
) -> serde_json::Map<String, serde_json::Value> {
    let mut version_meta = manifest_value.as_object().cloned().unwrap_or_default();
    version_meta.insert(
        "_id".into(),
        serde_json::json!(format!("{}@{}", package.name, package.version)),
    );
    version_meta.insert("dist".into(), dist.clone());
    version_meta
}

fn build_dist(
    registry: &str,
    package: &PackageIdentity,
    tarball_bytes: &[u8],
) -> serde_json::Value {
    let digest = sha1::Sha1::digest(tarball_bytes);
    let digest_bytes = digest.as_slice();

    // npm tarball URL shape: <registry>/<name>/-/<basename>-<version>.tgz
    serde_json::json!({
        "integrity": format!(
            "sha1-{}",
            base64::engine::general_purpose::STANDARD.encode(digest_bytes)
        ),
        "shasum": hex::encode(digest_bytes),
        "tarball": format!(
            "{}/{}/-/{}-{}.tgz",
            registry.trim_end_matches('/'),
            package.name,
            tarball_basename(&package.name),
            package.version
        ),
    })
}
