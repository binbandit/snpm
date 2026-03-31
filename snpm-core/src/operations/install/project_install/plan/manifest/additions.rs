use crate::Project;
use crate::registry::RegistryProtocol;

use std::collections::BTreeMap;

use super::super::super::super::manifest::parse_requested_with_protocol;

pub(crate) fn collect_additions(
    project: &Project,
    requested: &[String],
) -> (BTreeMap<String, String>, BTreeMap<String, RegistryProtocol>) {
    let (requested_ranges, requested_protocols) = parse_requested_with_protocol(requested);
    let mut additions = BTreeMap::new();
    let mut addition_protocols = BTreeMap::new();

    for (name, range) in requested_ranges {
        if project.manifest.dependencies.contains_key(&name)
            || project.manifest.dev_dependencies.contains_key(&name)
            || project.manifest.optional_dependencies.contains_key(&name)
        {
            continue;
        }

        additions.insert(name.clone(), range);
        if let Some(protocol) = requested_protocols.get(&name) {
            addition_protocols.insert(name, protocol.clone());
        }
    }

    (additions, addition_protocols)
}
