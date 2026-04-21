use super::*;
use crate::registry::RegistryProtocol;

use std::collections::BTreeMap;

#[test]
fn build_dep_request_no_overrides() {
    let protocol = RegistryProtocol::npm();
    let request = build_dep_request("lodash", "^4.0.0", &protocol, None, None);
    assert_eq!(request.source, "lodash");
    assert_eq!(request.range, "^4.0.0");
}

#[test]
fn build_dep_request_with_override() {
    let protocol = RegistryProtocol::npm();
    let overrides = BTreeMap::from([("lodash".to_string(), "^5.0.0".to_string())]);
    let request = build_dep_request("lodash", "^4.0.0", &protocol, Some(&overrides), None);
    assert_eq!(request.source, "lodash");
    assert_eq!(request.range, "^5.0.0");
}

#[test]
fn build_dep_request_override_not_matching() {
    let protocol = RegistryProtocol::npm();
    let overrides = BTreeMap::from([("react".to_string(), "^18.0.0".to_string())]);
    let request = build_dep_request("lodash", "^4.0.0", &protocol, Some(&overrides), None);
    assert_eq!(request.source, "lodash");
    assert_eq!(request.range, "^4.0.0");
}

#[test]
fn build_dep_request_override_with_protocol() {
    let protocol = RegistryProtocol::npm();
    let overrides = BTreeMap::from([("pkg".to_string(), "npm:other-pkg@^2.0.0".to_string())]);
    let request = build_dep_request("pkg", "^1.0.0", &protocol, Some(&overrides), None);
    assert_eq!(request.source, "other-pkg");
    assert_eq!(request.range, "^2.0.0");
}

#[test]
fn build_dep_request_matches_selector_override() {
    let protocol = RegistryProtocol::npm();
    let overrides = BTreeMap::from([(
        "make-fetch-happen@npm:^14.0.1".to_string(),
        "portal:packages/make-fetch-smaller".to_string(),
    )]);
    let request = build_dep_request(
        "make-fetch-happen",
        "^14.0.1",
        &protocol,
        Some(&overrides),
        None,
    );
    assert_eq!(request.protocol, RegistryProtocol::file());
    assert_eq!(request.source, "packages/make-fetch-smaller");
    assert_eq!(request.range, "latest");
}

#[test]
fn build_dep_request_package_less_npm_protocol_uses_dependency_name() {
    let protocol = RegistryProtocol::npm();
    let request = build_dep_request(
        "brace-expansion",
        "npm:^1.1.7",
        &protocol,
        None,
        None,
    );
    assert_eq!(request.protocol, RegistryProtocol::npm());
    assert_eq!(request.source, "brace-expansion");
    assert_eq!(request.range, "^1.1.7");
}

#[test]
fn build_dep_request_workspace_protocol_uses_dependency_name() {
    let protocol = RegistryProtocol::npm();
    let request = build_dep_request("@yarnpkg/core", "workspace:^", &protocol, None, None);
    assert_eq!(request.protocol, RegistryProtocol::npm());
    assert_eq!(request.source, "@yarnpkg/core");
    assert_eq!(request.range, "*");
}

#[test]
fn build_dep_request_git_shorthand_uses_spec_as_source() {
    let protocol = RegistryProtocol::git();
    let request = build_dep_request("tooling", "webpack/tooling#v1.26.1", &protocol, None, None);
    assert_eq!(request.source, "webpack/tooling#v1.26.1");
    assert_eq!(request.range, "latest");
}

#[test]
fn split_protocol_spec_npm_scoped() {
    let result = split_protocol_spec("npm:@scope/pkg@^1.0.0");
    let (protocol, source, range) = result.unwrap();
    assert_eq!(protocol, RegistryProtocol::npm());
    assert_eq!(source, "@scope/pkg");
    assert_eq!(range, "^1.0.0");
}

#[test]
fn split_protocol_spec_npm_unscoped() {
    let result = split_protocol_spec("npm:lodash@^4.0.0");
    let (protocol, source, range) = result.unwrap();
    assert_eq!(protocol, RegistryProtocol::npm());
    assert_eq!(source, "lodash");
    assert_eq!(range, "^4.0.0");
}

#[test]
fn split_protocol_spec_jsr() {
    let result = split_protocol_spec("jsr:@std/path@^1.0.0");
    let (protocol, source, range) = result.unwrap();
    assert_eq!(protocol, RegistryProtocol::jsr());
    assert_eq!(source, "@std/path");
    assert_eq!(range, "^1.0.0");
}

#[test]
fn split_protocol_spec_no_version() {
    let result = split_protocol_spec("npm:lodash");
    let (protocol, source, range) = result.unwrap();
    assert_eq!(protocol, RegistryProtocol::npm());
    assert_eq!(source, "lodash");
    assert_eq!(range, "latest");
}

#[test]
fn split_protocol_spec_git() {
    let result = split_protocol_spec("git+https://github.com/foo/bar.git");
    let (protocol, source, _range) = result.unwrap();
    assert_eq!(protocol, RegistryProtocol::git());
    assert!(source.contains("github.com"));
}

#[test]
fn split_protocol_spec_github_prefix() {
    let result = split_protocol_spec("github:foo/bar#v1.0.0");
    let (protocol, source, range) = result.unwrap();
    assert_eq!(protocol, RegistryProtocol::git());
    assert_eq!(source, "github:foo/bar#v1.0.0");
    assert_eq!(range, "latest");
}

#[test]
fn split_protocol_spec_empty_rest_returns_none() {
    assert!(split_protocol_spec("npm:").is_none());
}

#[test]
fn split_protocol_spec_no_colon_returns_none() {
    assert!(split_protocol_spec("lodash").is_none());
}
