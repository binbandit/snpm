use crate::registry::RegistryProtocol;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct DepRequest {
    pub source: String,
    pub range: String,
    pub protocol: RegistryProtocol,
}

pub fn build_dep_request(
    name: &str,
    range: &str,
    protocol: &RegistryProtocol,
    overrides: Option<&BTreeMap<String, String>>,
) -> DepRequest {
    let overridden = overrides
        .and_then(|map| map.get(name))
        .map(|s| s.as_str())
        .unwrap_or(range);

    if let Some((proto, source, semver_range)) = split_protocol_spec(overridden) {
        DepRequest {
            source,
            range: semver_range,
            protocol: proto,
        }
    } else {
        DepRequest {
            source: name.to_string(),
            range: overridden.to_string(),
            protocol: protocol.clone(),
        }
    }
}

pub fn split_protocol_spec(spec: &str) -> Option<(RegistryProtocol, String, String)> {
    let colon = spec.find(':')?;
    let (prefix, rest) = spec.split_at(colon);
    let rest = &rest[1..];

    if prefix.is_empty() || rest.is_empty() {
        return None;
    }

    if is_git_protocol_prefix(prefix) {
        let source = format!("{prefix}:{rest}");
        return Some((RegistryProtocol::git(), source, "latest".to_string()));
    }

    let protocol = match prefix {
        "npm" => RegistryProtocol::npm(),
        "jsr" => RegistryProtocol::jsr(),
        other => RegistryProtocol::custom(other),
    };

    let mut source = rest.to_string();
    let mut range = "latest".to_string();

    if let Some(at) = rest.rfind('@') {
        let (name, ver_part) = rest.split_at(at);
        if !name.is_empty() {
            source = name.to_string();
        }
        let ver = ver_part.trim_start_matches('@');
        if !ver.is_empty() {
            range = ver.to_string()
        }
    }

    Some((protocol, source, range))
}

fn is_git_protocol_prefix(prefix: &str) -> bool {
    matches!(
        prefix,
        "git"
            | "git+http"
            | "git+https"
            | "git+rsync"
            | "git+ftp"
            | "git+file"
            | "git+ssh"
            | "ssh"
    )
}
