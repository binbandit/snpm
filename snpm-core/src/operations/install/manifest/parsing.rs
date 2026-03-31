use crate::operations::install::utils::ParsedSpec;
use crate::registry::RegistryProtocol;
use std::collections::BTreeMap;

pub fn parse_requested_with_protocol(
    specs: &[String],
) -> (BTreeMap<String, String>, BTreeMap<String, RegistryProtocol>) {
    let mut ranges = BTreeMap::new();
    let mut protocols = BTreeMap::new();

    for spec in specs {
        let parsed = parse_requested_spec(spec);
        ranges.insert(parsed.name.clone(), parsed.range.clone());

        if let Some(protocol_str) = parsed.protocol.as_deref() {
            let protocol = match protocol_str {
                "npm" => RegistryProtocol::npm(),
                "git" => RegistryProtocol::git(),
                "jsr" => RegistryProtocol::jsr(),
                other if other.starts_with("git+") => RegistryProtocol::git(),
                other => RegistryProtocol::custom(other),
            };
            protocols.insert(parsed.name.clone(), protocol);
        }
    }

    (ranges, protocols)
}

pub fn parse_requested_spec(spec: &str) -> ParsedSpec {
    let (protocol, rest) = split_protocol_prefix(spec);
    let (name, range) = split_package_spec(rest);

    ParsedSpec {
        name,
        range,
        protocol,
    }
}

pub fn parse_spec(spec: &str) -> (String, String) {
    split_package_spec(spec)
}

fn split_protocol_prefix(spec: &str) -> (Option<String>, &str) {
    let Some(index) = spec.find(':') else {
        return (None, spec);
    };

    let (prefix, after) = spec.split_at(index);
    if prefix.is_empty() {
        (None, spec)
    } else {
        (Some(prefix.to_string()), &after[1..])
    }
}

fn split_package_spec(spec: &str) -> (String, String) {
    if let Some(without_at) = spec.strip_prefix('@') {
        if let Some(index) = without_at.rfind('@') {
            let (scope_and_name, range) = without_at.split_at(index);
            return (
                format!("@{}", scope_and_name),
                range.trim_start_matches('@').to_string(),
            );
        }

        return (spec.to_string(), "latest".to_string());
    }

    if let Some(index) = spec.rfind('@') {
        let (name, range) = spec.split_at(index);
        (name.to_string(), range.trim_start_matches('@').to_string())
    } else {
        (spec.to_string(), "latest".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_spec_simple() {
        let (name, range) = parse_spec("lodash@^4.0.0");
        assert_eq!(name, "lodash");
        assert_eq!(range, "^4.0.0");
    }

    #[test]
    fn parse_spec_scoped() {
        let (name, range) = parse_spec("@types/node@^18.0.0");
        assert_eq!(name, "@types/node");
        assert_eq!(range, "^18.0.0");
    }

    #[test]
    fn parse_spec_no_version() {
        let (name, range) = parse_spec("lodash");
        assert_eq!(name, "lodash");
        assert_eq!(range, "latest");
    }

    #[test]
    fn parse_spec_scoped_no_version() {
        let (name, range) = parse_spec("@types/node");
        assert_eq!(name, "@types/node");
        assert_eq!(range, "latest");
    }

    #[test]
    fn parse_requested_spec_with_protocol() {
        let parsed = parse_requested_spec("npm:@scope/pkg@^1.0.0");
        assert_eq!(parsed.protocol.as_deref(), Some("npm"));
        assert_eq!(parsed.name, "@scope/pkg");
        assert_eq!(parsed.range, "^1.0.0");
    }

    #[test]
    fn parse_requested_spec_no_protocol() {
        let parsed = parse_requested_spec("lodash@^4.0.0");
        assert_eq!(parsed.name, "lodash");
        assert_eq!(parsed.range, "^4.0.0");
    }
}
