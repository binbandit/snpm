use crate::{Project, Result};

use std::path::PathBuf;

use super::walk::collect_dir_files_filtered;

pub(super) fn collect_default_files(project: &Project, files: &mut Vec<PathBuf>) -> Result<()> {
    collect_dir_files_filtered(&project.root, files, project)
}
