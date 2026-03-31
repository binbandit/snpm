mod manifest;
mod payload;
mod request;

use crate::{Project, Result, SnpmConfig, console};

use std::path::Path;

use manifest::{PackageIdentity, publish_identity, read_manifest_value};
use payload::build_publish_payload;
use request::send_publish_request;

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
    let package = publish_identity(project)?;

    if options.dry_run {
        log_dry_run(&package, options);
        return Ok(());
    }

    let manifest_value = read_manifest_value(project)?;
    let payload = build_publish_payload(config, &package, &manifest_value, tarball_path, options)?;

    console::step(&format!("Publishing {}@{}", package.name, package.version));

    send_publish_request(config, &package, options, payload).await?;

    console::info(&format!(
        "Published {}@{} with tag \"{}\"",
        package.name, package.version, options.tag
    ));

    Ok(())
}

fn log_dry_run(package: &PackageIdentity, options: &PublishOptions) {
    console::info(&format!(
        "Dry run: would publish {}@{} with tag \"{}\"",
        package.name, package.version, options.tag
    ));
}
