use super::*;
use crate::registry::RegistryProtocol;

use std::path::{Path, PathBuf};

#[test]
fn normalize_dependency_range_resolves_local_file_dependencies() {
    let range = normalize_dependency_range("file:///packages/foo", "file:../bar");
    assert_eq!(range, "file:/packages/bar");
}

#[test]
fn normalize_dependency_range_leaves_registry_dependencies_unchanged() {
    let range = normalize_dependency_range("https://registry.example/pkg.tgz", "^1.0.0");
    assert_eq!(range, "^1.0.0");
}

#[test]
fn resolve_relative_path_simple() {
    let base = Path::new("/packages/foo");
    let result = resolve_relative_path(base, "bar/baz");
    assert_eq!(result, PathBuf::from("/packages/foo/bar/baz"));
}

#[test]
fn resolve_relative_path_with_parent() {
    let base = Path::new("/packages/foo");
    let result = resolve_relative_path(base, "../sibling");
    assert_eq!(result, PathBuf::from("/packages/sibling"));
}

#[test]
fn resolve_relative_path_with_curdir() {
    let base = Path::new("/packages/foo");
    let result = resolve_relative_path(base, "./bar");
    assert_eq!(result, PathBuf::from("/packages/foo/bar"));
}

#[test]
fn resolve_relative_path_root_resets() {
    let base = Path::new("/packages/foo");
    let result = resolve_relative_path(base, "/absolute/path");
    assert_eq!(result, PathBuf::from("/absolute/path"));
}

#[test]
fn resolve_relative_path_multiple_parent_dirs() {
    let base = Path::new("/a/b/c/d");
    let result = resolve_relative_path(base, "../../e");
    assert_eq!(result, PathBuf::from("/a/b/e"));
}

#[test]
fn protocol_from_range_file() {
    assert_eq!(
        protocol_from_range("file:../local"),
        RegistryProtocol::file()
    );
}

#[test]
fn protocol_from_range_git_colon() {
    assert_eq!(protocol_from_range("git:repo.git"), RegistryProtocol::git());
}

#[test]
fn protocol_from_range_git_plus() {
    assert_eq!(
        protocol_from_range("git+https://github.com/foo/bar.git"),
        RegistryProtocol::git()
    );
}

#[test]
fn protocol_from_range_jsr() {
    assert_eq!(
        protocol_from_range("jsr:@scope/pkg@^1.0.0"),
        RegistryProtocol::jsr()
    );
}

#[test]
fn protocol_from_range_npm_default() {
    assert_eq!(protocol_from_range("^1.0.0"), RegistryProtocol::npm());
}

#[test]
fn protocol_from_range_semver_range() {
    assert_eq!(
        protocol_from_range(">=2.0.0 <3.0.0"),
        RegistryProtocol::npm()
    );
}
