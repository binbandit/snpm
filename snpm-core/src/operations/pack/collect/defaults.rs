use crate::{Project, Result};

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::CollectedFile;
use super::walk::collect_dir_files_filtered;

pub(super) fn collect_default_files(
    project: &Project,
    files: &mut BTreeMap<PathBuf, CollectedFile>,
) -> Result<()> {
    collect_dir_files_filtered(&project.root, files, project, true)
}
