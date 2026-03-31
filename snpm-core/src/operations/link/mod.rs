mod global;
mod local;
mod symlinks;

use crate::{Project, Result, SnpmError};

pub use global::{link_global, unlink_global};
pub use local::{link_local, unlink_local};

fn package_name<'a>(project: &'a Project, action: &str) -> Result<&'a str> {
    project
        .manifest
        .name
        .as_deref()
        .ok_or_else(|| SnpmError::ManifestInvalid {
            path: project.manifest_path.clone(),
            reason: format!("package.json must have a \"name\" field to {}", action),
        })
}
