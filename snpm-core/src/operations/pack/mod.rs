mod collect;
mod safety;
mod tarball;

use crate::{Project, Result, SnpmError};
use serde::Serialize;
use std::path::{Path, PathBuf};

use collect::collect_pack_files;
use safety::audit_pack;
use tarball::write_tarball;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PackFileReason {
    Manifest,
    ManifestFiles,
    DefaultScan,
    Mandatory,
    MainEntry,
    BinEntry,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    pub path: String,
    pub size: u64,
    pub reason: PackFileReason,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PackFindingSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackFinding {
    pub code: String,
    pub severity: PackFindingSeverity,
    pub path: Option<String>,
    pub message: String,
}

impl PackFinding {
    pub fn is_blocking(&self) -> bool {
        self.severity == PackFindingSeverity::Error
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackInspection {
    pub name: String,
    pub version: String,
    pub files: Vec<PackFile>,
    pub unpacked_size: u64,
    pub findings: Vec<PackFinding>,
    #[serde(skip)]
    archive_paths: Vec<PathBuf>,
}

impl PackInspection {
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackResult {
    pub tarball_path: PathBuf,
    pub packed_size: u64,
    pub inspection: PackInspection,
}

impl PackResult {
    pub fn file_count(&self) -> usize {
        self.inspection.file_count()
    }
}

pub fn inspect_pack(project: &Project) -> Result<PackInspection> {
    let name = required_manifest_field(project, project.manifest.name.as_deref(), "name")?;
    let version = required_manifest_field(project, project.manifest.version.as_deref(), "version")?;
    let collected = collect_pack_files(project)?;
    let unpacked_size = collected.iter().map(|file| file.size).sum();
    let archive_paths = collected
        .iter()
        .map(|file| file.absolute_path.clone())
        .collect();
    let files = collected
        .iter()
        .map(|file| PackFile {
            path: file.relative_path.clone(),
            size: file.size,
            reason: file.reason.clone(),
        })
        .collect();
    let findings = audit_pack(project, &collected)?;

    Ok(PackInspection {
        name: name.to_string(),
        version: version.to_string(),
        files,
        unpacked_size,
        findings,
        archive_paths,
    })
}

pub fn pack(project: &Project, output_dir: &Path) -> Result<PackResult> {
    let inspection = inspect_pack(project)?;
    let safe_name = inspection.name.replace('/', "-").replace('@', "");
    let tarball_name = format!("{}-{}.tgz", safe_name, inspection.version);
    let tarball_path = output_dir.join(&tarball_name);
    let packed_size = write_tarball(
        output_dir,
        &tarball_path,
        &project.root,
        &inspection.archive_paths,
    )?;

    Ok(PackResult {
        tarball_path,
        packed_size,
        inspection,
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

#[cfg(test)]
mod tests;
