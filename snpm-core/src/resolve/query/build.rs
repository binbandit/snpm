use crate::registry::RegistryProtocol;

use std::collections::BTreeMap;

use super::{DepRequest, split_protocol_spec};

pub fn build_dep_request(
    name: &str,
    range: &str,
    protocol: &RegistryProtocol,
    overrides: Option<&BTreeMap<String, String>>,
    workspace_sources: Option<&BTreeMap<String, String>>,
) -> DepRequest {
    let overridden = select_override(name, range, protocol, overrides).unwrap_or(range);

    if let Some(source) = resolve_local_workspace_source(name, overridden, workspace_sources) {
        DepRequest {
            source,
            range: "latest".to_string(),
            protocol: RegistryProtocol::file(),
        }
    } else if let Some(range) = split_workspace_spec(overridden) {
        DepRequest {
            source: name.to_string(),
            range: range.to_string(),
            protocol: RegistryProtocol::npm(),
        }
    } else if let Some((protocol, range)) = split_package_less_registry_spec(overridden) {
        DepRequest {
            source: name.to_string(),
            range: range.to_string(),
            protocol,
        }
    } else if let Some((protocol, source, range)) = split_protocol_spec(overridden) {
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

fn select_override<'a>(
    name: &str,
    range: &str,
    protocol: &RegistryProtocol,
    overrides: Option<&'a BTreeMap<String, String>>,
) -> Option<&'a str> {
    let overrides = overrides?;

    overrides
        .get(name)
        .or_else(|| overrides.get(&format!("{name}@{}:{range}", protocol.name)))
        .or_else(|| overrides.get(&format!("{name}@{range}")))
        .map(|value| value.as_str())
}

fn split_package_less_registry_spec(spec: &str) -> Option<(RegistryProtocol, &str)> {
    for (prefix, protocol) in [
        ("npm:", RegistryProtocol::npm()),
        ("jsr:", RegistryProtocol::jsr()),
    ] {
        let Some(rest) = spec.strip_prefix(prefix) else {
            continue;
        };

        if rest.is_empty() {
            return Some((protocol, "latest"));
        }

        if looks_like_registry_range(rest) {
            return Some((protocol, rest));
        }
    }

    None
}

fn split_workspace_spec(spec: &str) -> Option<&str> {
    let rest = spec.strip_prefix("workspace:")?.trim();
    Some(match rest {
        "" | "*" | "^" | "~" => "*",
        other => other,
    })
}

fn resolve_local_workspace_source(
    name: &str,
    spec: &str,
    workspace_sources: Option<&BTreeMap<String, String>>,
) -> Option<String> {
    if !spec.starts_with("workspace:") {
        return None;
    }

    workspace_sources?.get(name).cloned()
}

fn looks_like_registry_range(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('@')
        && !value.contains('@')
        && !value.contains('/')
        && !value.contains(':')
}
