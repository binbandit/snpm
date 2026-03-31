use crate::Project;
use crate::resolve::{ResolutionGraph, RootDependency};

pub(super) fn filter_root_dependencies<'a>(
    project: &Project,
    graph: &'a ResolutionGraph,
    include_dev: bool,
) -> Vec<(&'a String, &'a RootDependency)> {
    let deps = &project.manifest.dependencies;
    let dev_deps = &project.manifest.dev_dependencies;
    let optional_deps = &project.manifest.optional_dependencies;

    graph
        .root
        .dependencies
        .iter()
        .filter(|(name, _dep)| {
            if !deps.contains_key(*name)
                && !dev_deps.contains_key(*name)
                && !optional_deps.contains_key(*name)
            {
                return false;
            }

            let only_dev = dev_deps.contains_key(*name) && !deps.contains_key(*name);
            if !include_dev && only_dev {
                return false;
            }

            true
        })
        .collect()
}
