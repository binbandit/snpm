use super::plan::ProjectInstallPlan;
use crate::console;
use crate::resolve::ResolutionGraph;

use crate::operations::install::utils::InstallOptions;

pub(super) fn print_install_changes(
    graph: &ResolutionGraph,
    plan: &ProjectInstallPlan,
    options: &InstallOptions,
) {
    if plan.additions.is_empty() && !plan.is_fresh_install {
        return;
    }

    println!();

    let mut packages_to_show = Vec::new();

    if !plan.additions.is_empty() {
        for name in plan.additions.keys() {
            if let Some(dep) = graph.root.dependencies.get(name) {
                packages_to_show.push((name.clone(), dep.resolved.version.clone(), options.dev));
            }
        }
    } else if plan.is_fresh_install {
        for (name, dep) in &graph.root.dependencies {
            let is_dev = plan.local_dev_deps.contains(name) && !plan.local_deps.contains(name);
            packages_to_show.push((name.clone(), dep.resolved.version.clone(), is_dev));
        }
    }

    packages_to_show.sort_by(|left, right| left.0.cmp(&right.0));

    for (name, version, is_dev) in packages_to_show {
        console::added(&name, &version, is_dev);
    }
}
