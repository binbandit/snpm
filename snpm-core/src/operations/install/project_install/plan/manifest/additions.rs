use crate::registry::RegistryProtocol;

use std::collections::BTreeMap;

use super::super::super::super::manifest::parse_requested_with_protocol;

// An explicitly requested package is always an addition/update, even
// when the manifest already lists it: `snpm add lodash@^4` must update
// an existing `"lodash": "^3.0.0"` entry (and `snpm add -D x` must be
// able to move x between sections) rather than silently doing nothing.
pub(crate) fn collect_additions(
    requested: &[String],
) -> (BTreeMap<String, String>, BTreeMap<String, RegistryProtocol>) {
    let (requested_ranges, requested_protocols) = parse_requested_with_protocol(requested);
    let mut additions = BTreeMap::new();
    let mut addition_protocols = BTreeMap::new();

    for (name, range) in requested_ranges {
        additions.insert(name.clone(), range);
        if let Some(protocol) = requested_protocols.get(&name) {
            addition_protocols.insert(name, protocol.clone());
        }
    }

    (additions, addition_protocols)
}
