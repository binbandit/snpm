use super::check::check_integrity_path;
use super::content::{integrity_content, legacy_integrity_content};
use super::write::write_integrity_path;
use crate::operations::install::utils::NO_PATCH_HASH;
use crate::operations::install::utils::types::IntegrityState;

use tempfile::tempdir;

#[test]
fn integrity_content_format() {
    let state = IntegrityState {
        lockfile_hash: "abc123".to_string(),
        patch_hash: "def456".to_string(),
    };
    assert_eq!(
        integrity_content(&state),
        "lockfile: abc123\npatches: def456\n"
    );
}

#[test]
fn legacy_integrity_content_format() {
    let state = IntegrityState {
        lockfile_hash: "abc123".to_string(),
        patch_hash: NO_PATCH_HASH.to_string(),
    };
    assert_eq!(legacy_integrity_content(&state), "lockfile: abc123\n");
}

#[test]
fn write_and_check_integrity() {
    let dir = tempdir().unwrap();
    let node_modules = dir.path().join("node_modules");
    std::fs::create_dir_all(&node_modules).unwrap();

    let state = IntegrityState {
        lockfile_hash: "test-hash".to_string(),
        patch_hash: "test-patch".to_string(),
    };

    write_integrity_path(&node_modules, &state).unwrap();
    assert!(check_integrity_path(&node_modules, &state));
}

#[test]
fn check_integrity_returns_false_when_missing() {
    let dir = tempdir().unwrap();
    let node_modules = dir.path().join("node_modules");
    std::fs::create_dir_all(&node_modules).unwrap();

    let state = IntegrityState {
        lockfile_hash: "test-hash".to_string(),
        patch_hash: NO_PATCH_HASH.to_string(),
    };

    assert!(!check_integrity_path(&node_modules, &state));
}

#[test]
fn check_integrity_accepts_legacy_format_when_no_patches() {
    let dir = tempdir().unwrap();
    let node_modules = dir.path().join("node_modules");
    std::fs::create_dir_all(&node_modules).unwrap();

    let state = IntegrityState {
        lockfile_hash: "test-hash".to_string(),
        patch_hash: NO_PATCH_HASH.to_string(),
    };

    std::fs::write(
        node_modules.join(".snpm-integrity"),
        "lockfile: test-hash\n",
    )
    .unwrap();
    assert!(check_integrity_path(&node_modules, &state));
}

#[test]
fn check_integrity_rejects_legacy_format_when_patches_exist() {
    let dir = tempdir().unwrap();
    let node_modules = dir.path().join("node_modules");
    std::fs::create_dir_all(&node_modules).unwrap();

    let state = IntegrityState {
        lockfile_hash: "test-hash".to_string(),
        patch_hash: "some-patch-hash".to_string(),
    };

    std::fs::write(
        node_modules.join(".snpm-integrity"),
        "lockfile: test-hash\n",
    )
    .unwrap();
    assert!(!check_integrity_path(&node_modules, &state));
}

#[test]
fn check_integrity_rejects_wrong_hash() {
    let dir = tempdir().unwrap();
    let node_modules = dir.path().join("node_modules");
    std::fs::create_dir_all(&node_modules).unwrap();

    let state = IntegrityState {
        lockfile_hash: "correct-hash".to_string(),
        patch_hash: NO_PATCH_HASH.to_string(),
    };

    std::fs::write(
        node_modules.join(".snpm-integrity"),
        "lockfile: wrong-hash\npatches: none\n",
    )
    .unwrap();
    assert!(!check_integrity_path(&node_modules, &state));
}

#[test]
fn write_integrity_skips_nonexistent_dir() {
    let dir = tempdir().unwrap();
    let node_modules = dir.path().join("nonexistent/node_modules");
    let state = IntegrityState {
        lockfile_hash: "hash".to_string(),
        patch_hash: NO_PATCH_HASH.to_string(),
    };
    write_integrity_path(&node_modules, &state).unwrap();
}
