use super::workspace::collect_workspace_protocol_deps;
use crate::Project;
use crate::project::Manifest;

use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn collect_workspace_protocol_deps_filters_correctly() {
    let project = Project {
        root: PathBuf::from("/tmp/project"),
        manifest_path: PathBuf::from("/tmp/project/package.json"),
        manifest: Manifest {
            name: Some("test".to_string()),
            version: None,
            private: false,
            dependencies: BTreeMap::from([
                ("lib-a".to_string(), "workspace:*".to_string()),
                ("lodash".to_string(), "^4.0.0".to_string()),
            ]),
            dev_dependencies: BTreeMap::from([
                ("lib-b".to_string(), "workspace:^".to_string()),
                ("jest".to_string(), "^29.0.0".to_string()),
            ]),
            optional_dependencies: BTreeMap::from([(
                "lib-c".to_string(),
                "workspace:~".to_string(),
            )]),
            scripts: BTreeMap::new(),
            files: None,
            bin: None,
            main: None,
            pnpm: None,
            snpm: None,
            workspaces: None,
        },
    };

    let (deps, dev_deps, optional_deps) = collect_workspace_protocol_deps(&project);
    assert!(deps.contains("lib-a"));
    assert!(!deps.contains("lodash"));
    assert!(dev_deps.contains("lib-b"));
    assert!(!dev_deps.contains("jest"));
    assert!(optional_deps.contains("lib-c"));
}
