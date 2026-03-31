use super::types::RootSpecSet;
use std::collections::BTreeMap;

pub fn build_project_manifest_root(
    dependencies: &BTreeMap<String, String>,
    development_dependencies: &BTreeMap<String, String>,
    optional_dependencies: &BTreeMap<String, String>,
    include_dev: bool,
) -> BTreeMap<String, String> {
    build_project_root_specs(
        dependencies,
        development_dependencies,
        optional_dependencies,
        include_dev,
    )
    .required
}

pub fn build_project_root_specs(
    dependencies: &BTreeMap<String, String>,
    development_dependencies: &BTreeMap<String, String>,
    optional_dependencies: &BTreeMap<String, String>,
    include_dev: bool,
) -> RootSpecSet {
    let mut required = dependencies.clone();

    if include_dev {
        for (name, range) in development_dependencies {
            required.entry(name.clone()).or_insert(range.clone());
        }
    }

    let optional = optional_dependencies.clone();
    for name in optional.keys() {
        required.remove(name);
    }

    RootSpecSet { required, optional }
}

#[cfg(test)]
mod tests {
    use super::build_project_root_specs;
    use std::collections::BTreeMap;

    #[test]
    fn optional_dependencies_override_required_roots() {
        let dependencies = BTreeMap::from([("left-pad".to_string(), "^1.0.0".to_string())]);
        let optional = BTreeMap::from([("left-pad".to_string(), "^2.0.0".to_string())]);

        let root_specs =
            build_project_root_specs(&dependencies, &BTreeMap::new(), &optional, false);

        assert!(root_specs.required.is_empty());
        assert_eq!(
            root_specs.optional.get("left-pad").map(String::as_str),
            Some("^2.0.0")
        );
    }

    #[test]
    fn build_project_root_specs_includes_dev_when_flagged() {
        let deps = BTreeMap::from([("react".to_string(), "^18.0.0".to_string())]);
        let dev = BTreeMap::from([("jest".to_string(), "^29.0.0".to_string())]);

        let specs = build_project_root_specs(&deps, &dev, &BTreeMap::new(), true);
        assert!(specs.required.contains_key("react"));
        assert!(specs.required.contains_key("jest"));
    }

    #[test]
    fn build_project_root_specs_excludes_dev_when_not_flagged() {
        let deps = BTreeMap::from([("react".to_string(), "^18.0.0".to_string())]);
        let dev = BTreeMap::from([("jest".to_string(), "^29.0.0".to_string())]);

        let specs = build_project_root_specs(&deps, &dev, &BTreeMap::new(), false);
        assert!(specs.required.contains_key("react"));
        assert!(!specs.required.contains_key("jest"));
    }

    #[test]
    fn build_project_root_specs_deps_take_priority_over_dev() {
        let deps = BTreeMap::from([("shared".to_string(), "^1.0.0".to_string())]);
        let dev = BTreeMap::from([("shared".to_string(), "^2.0.0".to_string())]);

        let specs = build_project_root_specs(&deps, &dev, &BTreeMap::new(), true);
        assert_eq!(
            specs.required.get("shared").map(String::as_str),
            Some("^1.0.0")
        );
    }
}
