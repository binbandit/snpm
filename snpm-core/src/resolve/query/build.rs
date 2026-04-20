use crate::registry::RegistryProtocol;

use std::collections::BTreeMap;

use super::{DepRequest, split_protocol_spec};

pub fn build_dep_request(
    name: &str,
    range: &str,
    protocol: &RegistryProtocol,
    overrides: Option<&BTreeMap<String, String>>,
) -> DepRequest {
    let overridden = overrides
        .and_then(|map| map.get(name))
        .map(|value| value.as_str())
        .unwrap_or(range);

    if let Some((protocol, source, range)) = split_protocol_spec(overridden) {
        DepRequest {
            source,
            range,
            protocol,
        }
    } else if protocol.name == "git" {
        DepRequest {
            source: overridden.to_string(),
            range: "latest".to_string(),
            protocol: protocol.clone(),
        }
    } else {
        DepRequest {
            source: name.to_string(),
            range: overridden.to_string(),
            protocol: protocol.clone(),
        }
    }
}
