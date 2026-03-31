mod defaults;
mod manifest;
mod walk;

use crate::{Project, Result};

use std::path::PathBuf;

use defaults::collect_default_files;
use manifest::{add_main_entry, add_mandatory_files, collect_manifest_files};

pub(super) fn collect_pack_files(project: &Project) -> Result<Vec<PathBuf>> {
    let mut files = vec![project.manifest_path.clone()];

    if project.manifest.files.is_some() {
        collect_manifest_files(project, &mut files)?;
    } else {
        collect_default_files(project, &mut files)?;
    }

    add_mandatory_files(&project.root, &mut files)?;
    add_main_entry(&project.root, &mut files, project);

    files.sort();
    files.dedup();
    Ok(files)
}
