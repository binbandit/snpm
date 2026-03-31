mod collect;
mod tarball;

use crate::{Project, Result, SnpmError};
use std::path::{Path, PathBuf};

use collect::collect_pack_files;
use tarball::write_tarball;

pub struct PackResult {
    pub tarball_path: PathBuf,
    pub file_count: usize,
    pub size: u64,
    pub name: String,
    pub version: String,
}

pub fn pack(project: &Project, output_dir: &Path) -> Result<PackResult> {
    let name = required_manifest_field(project, project.manifest.name.as_deref(), "name")?;
    let version = required_manifest_field(project, project.manifest.version.as_deref(), "version")?;

    let safe_name = name.replace('/', "-").replace('@', "");
    let tarball_name = format!("{}-{}.tgz", safe_name, version);
    let tarball_path = output_dir.join(&tarball_name);
    let files = collect_pack_files(project)?;
    let size = write_tarball(output_dir, &tarball_path, &project.root, &files)?;

    Ok(PackResult {
        size,
        file_count: files.len(),
        tarball_path,
        name: name.to_string(),
        version: version.to_string(),
    })
}

fn required_manifest_field<'a>(
    project: &Project,
    value: Option<&'a str>,
    field: &str,
) -> Result<&'a str> {
    value.ok_or_else(|| SnpmError::ManifestInvalid {
        path: project.manifest_path.clone(),
        reason: format!("package.json must have a \"{field}\" field to pack"),
    })
}
