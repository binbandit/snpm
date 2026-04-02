use crate::Project;
use crate::project::Manifest;

use std::collections::BTreeMap;
use std::path::Path;

pub(super) fn temporary_project(temp_path: &Path, root_deps: BTreeMap<String, String>) -> Project {
    let manifest = Manifest {
        name: Some("dlx-project".to_string()),
        version: Some("0.0.0".to_string()),
        private: false,
        dependencies: root_deps,
        dev_dependencies: BTreeMap::new(),
        optional_dependencies: BTreeMap::new(),
        scripts: BTreeMap::new(),
        files: None,
        bin: None,
        main: None,
        pnpm: None,
        snpm: None,
        workspaces: None,
    };

    Project {
        root: temp_path.to_path_buf(),
        manifest_path: temp_path.join("package.json"),
        manifest,
    }
}
