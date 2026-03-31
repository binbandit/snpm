use crate::Project;

use std::collections::BTreeSet;

pub(super) fn collect_workspace_protocol_deps(
    project: &Project,
) -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
    let deps = project
        .manifest
        .dependencies
        .iter()
        .filter(|(_, value)| value.starts_with("workspace:"))
        .map(|(name, _)| name.clone())
        .collect();

    let dev_deps = project
        .manifest
        .dev_dependencies
        .iter()
        .filter(|(_, value)| value.starts_with("workspace:"))
        .map(|(name, _)| name.clone())
        .collect();

    let optional_deps = project
        .manifest
        .optional_dependencies
        .iter()
        .filter(|(_, value)| value.starts_with("workspace:"))
        .map(|(name, _)| name.clone())
        .collect();

    (deps, dev_deps, optional_deps)
}
