use crate::registry::RegistryProtocol;
use crate::workspace::CatalogConfig;
use crate::{Workspace, lockfile::CompatibleLockfile, operations::install::manifest::RootSpecSet};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub(in crate::operations::install::project_install) struct ProjectInstallPlan {
    pub(in crate::operations::install::project_install) workspace: Option<Workspace>,
    pub(in crate::operations::install::project_install) catalog: Option<CatalogConfig>,
    pub(in crate::operations::install::project_install) overrides: BTreeMap<String, String>,
    pub(in crate::operations::install::project_install) additions: BTreeMap<String, String>,
    pub(in crate::operations::install::project_install) local_deps: BTreeSet<String>,
    pub(in crate::operations::install::project_install) local_dev_deps: BTreeSet<String>,
    pub(in crate::operations::install::project_install) local_optional_deps: BTreeSet<String>,
    pub(in crate::operations::install::project_install) manifest_root: BTreeMap<String, String>,
    pub(in crate::operations::install::project_install) root_specs: RootSpecSet,
    pub(in crate::operations::install::project_install) root_dependencies: BTreeMap<String, String>,
    pub(in crate::operations::install::project_install) root_protocols:
        BTreeMap<String, RegistryProtocol>,
    pub(in crate::operations::install::project_install) optional_root_names: BTreeSet<String>,
    pub(in crate::operations::install::project_install) lockfile_path: PathBuf,
    pub(in crate::operations::install::project_install) compatible_lockfile:
        Option<CompatibleLockfile>,
    pub(in crate::operations::install::project_install) is_fresh_install: bool,
}

impl ProjectInstallPlan {
    pub(in crate::operations::install::project_install) fn workspace_root_label(&self) -> String {
        self.workspace
            .as_ref()
            .map(|workspace| workspace.root.display().to_string())
            .unwrap_or_else(|| "<none>".to_string())
    }

    pub(in crate::operations::install::project_install) fn lockfile_source_label(&self) -> String {
        if self.lockfile_path.is_file() {
            return self.lockfile_path.display().to_string();
        }

        self.compatible_lockfile
            .as_ref()
            .map(|source| source.path.display().to_string())
            .unwrap_or_else(|| self.lockfile_path.display().to_string())
    }
}
