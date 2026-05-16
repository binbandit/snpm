use super::copy::atomic_finalize_extracted_dir;
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

#[test]
fn atomic_finalize_renames_when_destination_is_missing() {
    let dir = tempdir().unwrap();
    let staged = dir.path().join(".snpm-extract-abc");
    let final_path = dir.path().join("pkg");
    fs::create_dir(&staged).unwrap();
    fs::write(staged.join("package.json"), "{}").unwrap();

    atomic_finalize_extracted_dir(&staged, &final_path).unwrap();

    assert!(!staged.exists(), "staged dir should be consumed by rename");
    assert!(final_path.join("package.json").is_file());
}

#[test]
fn atomic_finalize_discards_staging_when_marker_winner_exists() {
    let dir = tempdir().unwrap();
    let staged = dir.path().join(".snpm-extract-abc");
    let final_path = dir.path().join("pkg");
    fs::create_dir(&staged).unwrap();
    fs::write(staged.join("package.json"), "{\"name\":\"mine\"}").unwrap();
    fs::create_dir(&final_path).unwrap();
    fs::write(final_path.join("package.json"), "{\"name\":\"winner\"}").unwrap();
    fs::write(final_path.join(".snpm_complete"), "").unwrap();

    atomic_finalize_extracted_dir(&staged, &final_path).unwrap();

    assert!(
        !staged.exists(),
        "lost the race; staged dir should be discarded"
    );
    let preserved = fs::read_to_string(final_path.join("package.json")).unwrap();
    assert_eq!(preserved, "{\"name\":\"winner\"}");
}

#[test]
fn atomic_finalize_replaces_incomplete_destination() {
    let dir = tempdir().unwrap();
    let staged = dir.path().join(".snpm-extract-abc");
    let final_path = dir.path().join("pkg");
    fs::create_dir(&staged).unwrap();
    fs::write(staged.join("package.json"), "{\"version\":\"new\"}").unwrap();
    // Simulate a crashed prior extract: destination occupies the path but has
    // no completion marker.
    fs::create_dir(&final_path).unwrap();
    fs::write(final_path.join("half-written.bin"), [0_u8; 32]).unwrap();

    atomic_finalize_extracted_dir(&staged, &final_path).unwrap();

    assert!(!staged.exists());
    assert!(final_path.join("package.json").is_file());
    assert!(!final_path.join("half-written.bin").exists());
}

#[test]
fn atomic_finalize_treats_marker_directory_as_unmarked() {
    let dir = tempdir().unwrap();
    let staged = dir.path().join(".snpm-extract-abc");
    let final_path = dir.path().join("pkg");
    fs::create_dir(&staged).unwrap();
    fs::write(staged.join("package.json"), "{\"version\":\"new\"}").unwrap();
    fs::create_dir(&final_path).unwrap();
    // Corrupt marker — the marker is meant to be a regular file. If a previous
    // crashed extract somehow left a directory at that path, we must not treat
    // it as "another worker finished".
    fs::create_dir(final_path.join(".snpm_complete")).unwrap();

    atomic_finalize_extracted_dir(&staged, &final_path).unwrap();

    assert!(!staged.exists());
    assert!(final_path.join("package.json").is_file());
    assert!(!final_path.join(".snpm_complete").exists());
}

#[test]
fn atomic_finalize_replaces_when_destination_is_a_regular_file() {
    let dir = tempdir().unwrap();
    let staged = dir.path().join(".snpm-extract-abc");
    let final_path = dir.path().join("pkg");
    fs::create_dir(&staged).unwrap();
    fs::write(staged.join("package.json"), "{\"version\":\"new\"}").unwrap();
    // Some operator-level mishap (or a pre-existing artifact) put a stray
    // *file* where the package directory should live. The finalize should
    // recover and overwrite it instead of leaving the staged dir orphaned.
    fs::write(&final_path, "stray").unwrap();

    atomic_finalize_extracted_dir(&staged, &final_path).unwrap();

    assert!(!staged.exists());
    assert!(final_path.is_dir());
    assert!(final_path.join("package.json").is_file());
}

#[test]
fn atomic_finalize_marker_winner_keeps_winning_extract_intact() {
    let dir = tempdir().unwrap();
    let staged = dir.path().join(".snpm-extract-abc");
    let final_path = dir.path().join("pkg");
    fs::create_dir(&staged).unwrap();
    fs::write(staged.join("package.json"), "{\"version\":\"loser\"}").unwrap();
    fs::create_dir(&final_path).unwrap();
    fs::write(final_path.join("package.json"), "{\"version\":\"winner\"}").unwrap();
    fs::write(final_path.join("extra.txt"), "winner-only").unwrap();
    fs::write(final_path.join(".snpm_complete"), "").unwrap();

    atomic_finalize_extracted_dir(&staged, &final_path).unwrap();

    // Marker present → the winner's tree must be left intact, including
    // files that don't exist in the loser's staged copy.
    assert_eq!(
        fs::read_to_string(final_path.join("package.json")).unwrap(),
        "{\"version\":\"winner\"}"
    );
    assert!(final_path.join("extra.txt").is_file());
    assert!(!staged.exists());
}
