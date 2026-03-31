use crate::Project;

pub(in crate::operations::run) fn matches_filters(name: &str, filters: &[String]) -> bool {
    if filters.is_empty() {
        return true;
    }

    for filter in filters {
        if filter == name {
            return true;
        }

        if let Ok(pattern) = glob::Pattern::new(filter) {
            if pattern.matches(name) {
                return true;
            }
        } else if name.contains(filter) {
            return true;
        }
    }

    false
}

pub(in crate::operations::run) fn project_label(project: &Project) -> String {
    if let Some(name) = project.manifest.name.as_deref() {
        name.to_string()
    } else {
        project
            .root
            .file_name()
            .and_then(|os| os.to_str())
            .unwrap_or(".")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{matches_filters, project_label};
    use crate::Project;
    use crate::project::Manifest;

    use std::collections::BTreeMap;
    use tempfile::tempdir;

    #[test]
    fn matches_filters_empty_returns_true() {
        assert!(matches_filters("anything", &[]));
    }

    #[test]
    fn matches_filters_exact_match() {
        let filters = vec!["my-pkg".to_string()];
        assert!(matches_filters("my-pkg", &filters));
        assert!(!matches_filters("other-pkg", &filters));
    }

    #[test]
    fn matches_filters_glob_match() {
        let filters = vec!["@scope/*".to_string()];
        assert!(matches_filters("@scope/foo", &filters));
        assert!(!matches_filters("other", &filters));
    }

    #[test]
    fn matches_filters_substring_fallback() {
        let filters = vec!["[invalid".to_string()];
        assert!(matches_filters("contains-[invalid-here", &filters));
        assert!(!matches_filters("no-match", &filters));
    }

    #[test]
    fn matches_filters_no_substring_when_glob_valid() {
        let filters = vec!["foo".to_string()];
        assert!(matches_filters("foo", &filters));
        assert!(!matches_filters("my-foo-pkg", &filters));
    }

    #[test]
    fn matches_filters_multiple() {
        let filters = vec!["pkg-a".to_string(), "pkg-b".to_string()];
        assert!(matches_filters("pkg-a", &filters));
        assert!(matches_filters("pkg-b", &filters));
        assert!(!matches_filters("pkg-c", &filters));
    }

    #[test]
    fn project_label_uses_name() {
        let dir = tempdir().unwrap();
        let project = Project {
            root: dir.path().to_path_buf(),
            manifest_path: dir.path().join("package.json"),
            manifest: Manifest {
                name: Some("my-project".to_string()),
                version: None,
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
                optional_dependencies: BTreeMap::new(),
                scripts: BTreeMap::new(),
                files: None,
                bin: None,
                main: None,
                pnpm: None,
                snpm: None,
                workspaces: None,
            },
        };

        assert_eq!(project_label(&project), "my-project");
    }

    #[test]
    fn project_label_falls_back_to_dir_name() {
        let dir = tempdir().unwrap();
        let project = Project {
            root: dir.path().to_path_buf(),
            manifest_path: dir.path().join("package.json"),
            manifest: Manifest {
                name: None,
                version: None,
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
                optional_dependencies: BTreeMap::new(),
                scripts: BTreeMap::new(),
                files: None,
                bin: None,
                main: None,
                pnpm: None,
                snpm: None,
                workspaces: None,
            },
        };

        let label = project_label(&project);
        assert!(!label.is_empty());
    }
}
