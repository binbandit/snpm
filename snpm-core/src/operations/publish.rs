use crate::{Project, Result, SnpmConfig, SnpmError, console, http};
use base64::Engine;
use sha1::Digest as _;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

pub struct PublishOptions {
    pub tag: String,
    pub access: Option<String>,
    pub otp: Option<String>,
    pub dry_run: bool,
}

impl Default for PublishOptions {
    fn default() -> Self {
        Self {
            tag: "latest".to_string(),
            access: None,
            otp: None,
            dry_run: false,
        }
    }
}

pub async fn publish(
    config: &SnpmConfig,
    project: &Project,
    tarball_path: &Path,
    options: &PublishOptions,
) -> Result<()> {
    let name = project
        .manifest
        .name
        .as_deref()
        .ok_or_else(|| SnpmError::ManifestInvalid {
            path: project.manifest_path.clone(),
            reason: "package.json must have a \"name\" field to publish".into(),
        })?;

    let version = project
        .manifest
        .version
        .as_deref()
        .ok_or_else(|| SnpmError::ManifestInvalid {
            path: project.manifest_path.clone(),
            reason: "package.json must have a \"version\" field to publish".into(),
        })?;

    if options.dry_run {
        console::info(&format!(
            "Dry run: would publish {}@{} with tag \"{}\"",
            name, version, options.tag
        ));
        return Ok(());
    }

    let tarball_bytes = fs::read(tarball_path).map_err(|source| SnpmError::ReadFile {
        path: tarball_path.to_path_buf(),
        source,
    })?;

    let integrity = format!(
        "sha1-{}",
        base64::engine::general_purpose::STANDARD.encode(sha1::Sha1::digest(&tarball_bytes))
    );

    let tarball_b64 =
        base64::engine::general_purpose::STANDARD.encode(&tarball_bytes);

    let manifest_json = fs::read_to_string(&project.manifest_path).map_err(|source| {
        SnpmError::ReadFile {
            path: project.manifest_path.clone(),
            source,
        }
    })?;
    let manifest_value: serde_json::Value =
        serde_json::from_str(&manifest_json).map_err(|source| SnpmError::ParseJson {
            path: project.manifest_path.clone(),
            source,
        })?;

    let access = options.access.as_deref().unwrap_or("public");

    // Build version metadata by merging manifest with publish-specific fields
    let mut version_meta = manifest_value
        .as_object()
        .cloned()
        .unwrap_or_default();
    version_meta.insert("_id".into(), serde_json::json!(format!("{}@{}", name, version)));
    version_meta.insert(
        "dist".into(),
        serde_json::json!({
            "integrity": integrity,
            "shasum": hex_encode(&sha1::Sha1::digest(&tarball_bytes)),
            "tarball": format!("{}/-/{}-{}.tgz", config.default_registry.trim_end_matches('/'), name, version),
        }),
    );

    let attachment_name = format!("{}-{}.tgz", name, version);

    let payload = serde_json::json!({
        "_id": name,
        "name": name,
        "description": manifest_value.get("description").unwrap_or(&serde_json::Value::Null),
        "dist-tags": {
            options.tag.as_str(): version
        },
        "versions": {
            version: version_meta
        },
        "access": access,
        "_attachments": {
            attachment_name: {
                "content_type": "application/octet-stream",
                "data": tarball_b64,
                "length": tarball_bytes.len(),
            }
        }
    });

    let encoded_name = crate::protocols::encode_package_name(name);
    let url = format!(
        "{}/{}",
        config.default_registry.trim_end_matches('/'),
        encoded_name
    );

    let client = http::create_client()?;
    let mut request = client.put(&url).json(&payload);

    if let Some(header_value) = config.authorization_header_for_url(&url) {
        request = request.header("authorization", header_value);
    }

    if let Some(ref otp) = options.otp {
        request = request.header("npm-otp", otp);
    }

    console::step(&format!("Publishing {}@{}", name, version));

    let response = request.send().await.map_err(|source| SnpmError::Http {
        url: url.clone(),
        source,
    })?;

    let status = response.status();

    if status.is_success() {
        console::info(&format!(
            "Published {}@{} with tag \"{}\"",
            name, version, options.tag
        ));
        Ok(())
    } else {
        let body = response.text().await.unwrap_or_default();
        Err(SnpmError::PublishFailed {
            name: name.to_string(),
            version: version.to_string(),
            reason: format!("registry returned {} — {}", status.as_u16(), body),
        })
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{:02x}", b).unwrap();
    }
    s
}
