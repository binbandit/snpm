use crate::operations::install::workspace::is_local_workspace_dependency;
use crate::{Project, Result, Workspace};

use std::collections::BTreeSet;

pub(super) fn collect_workspace_protocol_deps(
    project: &Project,
    workspace: &Workspace,
) -> Result<(BTreeSet<String>, BTreeSet<String>, BTreeSet<String>)> {
    let deps = collect_local_deps(&project.manifest.dependencies, workspace)?;
    let dev_deps = collect_local_deps(&project.manifest.dev_dependencies, workspace)?;
    let optional_deps = collect_local_deps(&project.manifest.optional_dependencies, workspace)?;

    Ok((deps, dev_deps, optional_deps))
}

fn collect_local_deps(
    deps: &std::collections::BTreeMap<String, String>,
    workspace: &Workspace,
) -> Result<BTreeSet<String>> {
    let mut local = BTreeSet::new();

    for (name, spec) in deps {
        if is_local_workspace_dependency(workspace, name, spec)? {
            local.insert(name.clone());
        }
    }

    Ok(local)
}
