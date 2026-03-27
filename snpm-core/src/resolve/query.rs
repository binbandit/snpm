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
        "git" | "git+http" | "git+https" | "git+rsync" | "git+ftp" | "git+file" | "git+ssh" | "ssh"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_dep_request_no_overrides() {
        let protocol = RegistryProtocol::npm();
        let req = build_dep_request("lodash", "^4.0.0", &protocol, None);
        assert_eq!(req.source, "lodash");
        assert_eq!(req.range, "^4.0.0");
    }

    #[test]
    fn build_dep_request_with_override() {
        let protocol = RegistryProtocol::npm();
        let overrides = BTreeMap::from([("lodash".to_string(), "^5.0.0".to_string())]);
        let req = build_dep_request("lodash", "^4.0.0", &protocol, Some(&overrides));
        assert_eq!(req.source, "lodash");
        assert_eq!(req.range, "^5.0.0");
    }

    #[test]
    fn build_dep_request_override_not_matching() {
        let protocol = RegistryProtocol::npm();
        let overrides = BTreeMap::from([("react".to_string(), "^18.0.0".to_string())]);
        let req = build_dep_request("lodash", "^4.0.0", &protocol, Some(&overrides));
        assert_eq!(req.source, "lodash");
        assert_eq!(req.range, "^4.0.0");
    }

    #[test]
    fn build_dep_request_override_with_protocol() {
        let protocol = RegistryProtocol::npm();
        let overrides = BTreeMap::from([("pkg".to_string(), "npm:other-pkg@^2.0.0".to_string())]);
        let req = build_dep_request("pkg", "^1.0.0", &protocol, Some(&overrides));
        assert_eq!(req.source, "other-pkg");
        assert_eq!(req.range, "^2.0.0");
    }

    #[test]
    fn split_protocol_spec_npm_scoped() {
        let result = split_protocol_spec("npm:@scope/pkg@^1.0.0");
        let (proto, source, range) = result.unwrap();
        assert_eq!(proto, RegistryProtocol::npm());
        assert_eq!(source, "@scope/pkg");
        assert_eq!(range, "^1.0.0");
    }

    #[test]
    fn split_protocol_spec_npm_unscoped() {
        let result = split_protocol_spec("npm:lodash@^4.0.0");
        let (proto, source, range) = result.unwrap();
        assert_eq!(proto, RegistryProtocol::npm());
        assert_eq!(source, "lodash");
        assert_eq!(range, "^4.0.0");
    }

    #[test]
    fn split_protocol_spec_jsr() {
        let result = split_protocol_spec("jsr:@std/path@^1.0.0");
        let (proto, source, range) = result.unwrap();
        assert_eq!(proto, RegistryProtocol::jsr());
        assert_eq!(source, "@std/path");
        assert_eq!(range, "^1.0.0");
    }

    #[test]
    fn split_protocol_spec_no_version() {
        let result = split_protocol_spec("npm:lodash");
        let (proto, source, range) = result.unwrap();
        assert_eq!(proto, RegistryProtocol::npm());
        assert_eq!(source, "lodash");
        assert_eq!(range, "latest");
    }

    #[test]
    fn split_protocol_spec_git() {
        let result = split_protocol_spec("git+https://github.com/foo/bar.git");
        let (proto, source, _range) = result.unwrap();
        assert_eq!(proto, RegistryProtocol::git());
        assert!(source.contains("github.com"));
    }

    #[test]
    fn split_protocol_spec_empty_rest_returns_none() {
        assert!(split_protocol_spec("npm:").is_none());
    }

    #[test]
    fn split_protocol_spec_no_colon_returns_none() {
        assert!(split_protocol_spec("lodash").is_none());
    }
}
