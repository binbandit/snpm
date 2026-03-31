use crate::{Project, Result, SnpmError};

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::super::types::{PatchSession, patches_dir};
use super::filter::filter_session_marker;
use super::rewrite::rewrite_diff_paths;

pub fn create_patch(
    project: &Project,
    session: &PatchSession,
    modified_dir: &Path,
) -> Result<PathBuf> {
    let patches = patches_dir(project);
    fs::create_dir_all(&patches).map_err(|source| SnpmError::WriteFile {
        path: patches.clone(),
        source,
    })?;

    let safe_name = session.package_name.replace('/', "+");
    let patch_filename = format!("{}@{}.patch", safe_name, session.package_version);
    let patch_path = patches.join(&patch_filename);

    let diff_output = run_diff(&session.original_path, modified_dir, &session.package_name)?;
    if diff_output.trim().is_empty() {
        return Err(SnpmError::PatchCreate {
            name: session.package_name.clone(),
            reason: "no changes detected".to_string(),
        });
    }

    let filtered = filter_session_marker(&diff_output);
    fs::write(&patch_path, filtered).map_err(|source| SnpmError::WriteFile {
        path: patch_path.clone(),
        source,
    })?;

    Ok(patch_path)
}

pub(super) fn run_diff(original: &Path, modified: &Path, package_name: &str) -> Result<String> {
    let original = original.canonicalize().map_err(|source| SnpmError::Io {
        path: original.to_path_buf(),
        source,
    })?;
    let modified = modified.canonicalize().map_err(|source| SnpmError::Io {
        path: modified.to_path_buf(),
        source,
    })?;

    let output = Command::new("diff")
        .args(["-ruN"])
        .arg(&original)
        .arg(&modified)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|_| SnpmError::PatchCreate {
            name: package_name.to_string(),
            reason: "diff command not found - please install diffutils".to_string(),
        })?;

    match output.status.code() {
        Some(0 | 1) => Ok(rewrite_diff_paths(
            &String::from_utf8_lossy(&output.stdout),
            &original,
            &modified,
        )),
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(SnpmError::PatchCreate {
                name: package_name.to_string(),
                reason: if stderr.is_empty() {
                    "diff command failed".to_string()
                } else {
                    stderr
                },
            })
        }
    }
}
