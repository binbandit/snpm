use super::{package_root_dir, sanitize_name};
use crate::store::PACKAGE_METADATA_FILE;
use std::fs;
use tempfile::tempdir;

#[test]
fn package_root_dir_finds_standard_package_dir() {
    let temp = tempdir().unwrap();
    let pkg_dir = temp.path();
    fs::create_dir_all(pkg_dir.join("package")).unwrap();
    fs::write(pkg_dir.join("package/package.json"), "{}").unwrap();

    let root = package_root_dir(pkg_dir);
    assert_eq!(root, pkg_dir.join("package"));
}

#[test]
fn package_root_dir_finds_nonstandard_toplevel_dir() {
    let temp = tempdir().unwrap();
    let pkg_dir = temp.path();
    fs::create_dir_all(pkg_dir.join("node")).unwrap();
    fs::write(pkg_dir.join("node/package.json"), "{}").unwrap();
    fs::write(pkg_dir.join(".snpm_complete"), "").unwrap();

    let root = package_root_dir(pkg_dir);
    assert_eq!(root, pkg_dir.join("node"));
}

#[test]
fn package_root_dir_returns_pkg_dir_when_flat() {
    let temp = tempdir().unwrap();
    let pkg_dir = temp.path();
    fs::write(pkg_dir.join("package.json"), "{}").unwrap();
    fs::write(pkg_dir.join("index.js"), "").unwrap();

    let root = package_root_dir(pkg_dir);
    assert_eq!(root, pkg_dir.to_path_buf());
}

#[test]
fn package_root_dir_uses_store_metadata_hint() {
    let temp = tempdir().unwrap();
    let pkg_dir = temp.path();
    fs::create_dir_all(pkg_dir.join("docs")).unwrap();
    fs::create_dir_all(pkg_dir.join("body-parser")).unwrap();
    fs::write(
        pkg_dir.join(PACKAGE_METADATA_FILE),
        r#"{ "rootRelativePath": "body-parser" }"#,
    )
    .unwrap();

    let root = package_root_dir(pkg_dir);
    assert_eq!(root, pkg_dir.join("body-parser"));
}

#[test]
fn sanitize_name_simple() {
    assert_eq!(sanitize_name("lodash"), "lodash");
}

#[test]
fn sanitize_name_scoped() {
    assert_eq!(sanitize_name("@types/node"), "@types_node");
}

#[test]
fn sanitize_name_multiple_slashes() {
    assert_eq!(sanitize_name("a/b/c"), "a_b_c");
}
