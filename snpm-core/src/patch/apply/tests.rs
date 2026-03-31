use super::materialize_patch_target;
use std::fs;
use tempfile::tempdir;

#[cfg(unix)]
#[test]
fn materialize_patch_target_replaces_symlink_with_copy() {
    let temp = tempdir().expect("tempdir");
    let store = temp.path().join("store");
    let target = temp.path().join("virtual-store/package");

    fs::create_dir_all(&store).expect("create store");
    fs::write(store.join("index.js"), "module.exports = 'store';\n").expect("write store");

    fs::create_dir_all(target.parent().expect("parent")).expect("create parent");
    std::os::unix::fs::symlink(&store, &target).expect("create symlink");

    materialize_patch_target(&target, &store).expect("materialize");

    let metadata = fs::symlink_metadata(&target).expect("metadata");
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(
        fs::read_to_string(target.join("index.js")).expect("read target"),
        "module.exports = 'store';\n"
    );
}
