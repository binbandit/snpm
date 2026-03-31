use super::create::run_diff;
use super::filter::filter_session_marker;
use super::rewrite::{normalize_patch_path, rewrite_diff_paths};

use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::tempdir;

#[test]
fn rewrite_diff_paths_rebases_headers() {
    let original = Path::new("/tmp/original");
    let modified = Path::new("/tmp/modified");
    let diff = "\
--- /tmp/original/lib/index.js\t2026-03-15 13:00:00\n\
+++ /tmp/modified/lib/index.js\t2026-03-15 13:00:01\n\
@@ -1 +1 @@\n\
-old\n\
+new\n\
";

    let rewritten = rewrite_diff_paths(diff, original, modified);

    assert!(rewritten.contains("--- a/lib/index.js\t2026-03-15 13:00:00"));
    assert!(rewritten.contains("+++ b/lib/index.js\t2026-03-15 13:00:01"));
}

#[test]
fn generated_patch_applies_to_package_directory() {
    if !command_available("diff") || !command_available("patch") {
        return;
    }

    let temp = tempdir().expect("tempdir");
    let original = temp.path().join("original");
    let modified = temp.path().join("modified");
    let target = temp.path().join("target");
    let patch_path = temp.path().join("dep-opt.patch");

    fs::create_dir_all(original.join("lib")).expect("create original");
    fs::create_dir_all(modified.join("lib")).expect("create modified");

    fs::write(original.join("lib/index.js"), "module.exports = 'old';\n").expect("write");
    fs::write(modified.join("lib/index.js"), "module.exports = 'new';\n").expect("write");

    let diff = run_diff(&original, &modified, "dep-opt").expect("run diff");
    assert!(diff.contains("--- a/lib/index.js"));
    assert!(diff.contains("+++ b/lib/index.js"));

    fs::write(&patch_path, diff).expect("write patch");
    fs::create_dir_all(target.join("lib")).expect("create target");
    fs::write(target.join("lib/index.js"), "module.exports = 'old';\n").expect("write");

    super::super::apply_patch(&target, &patch_path).expect("apply patch");

    let patched = fs::read_to_string(target.join("lib/index.js")).expect("read target");
    assert_eq!(patched, "module.exports = 'new';\n");
}

#[test]
fn filter_session_marker_removes_marker_section() {
    let content = "\
diff -ruN a/lib/index.js b/lib/index.js
--- a/lib/index.js
+++ b/lib/index.js
@@ -1 +1 @@
-old
+new
diff -ruN a/.snpm_patch_session b/.snpm_patch_session
--- a/.snpm_patch_session
+++ b/.snpm_patch_session
@@ -0,0 +1 @@
+session data
diff -ruN a/lib/other.js b/lib/other.js
--- a/lib/other.js
+++ b/lib/other.js
@@ -1 +1 @@
-other old
+other new
";
    let filtered = filter_session_marker(content);
    assert!(filtered.contains("lib/index.js"));
    assert!(filtered.contains("lib/other.js"));
    assert!(!filtered.contains(".snpm_patch_session"));
}

#[test]
fn filter_session_marker_no_marker() {
    let content = "diff -ruN a/file b/file\n--- a/file\n+++ b/file\n";
    let filtered = filter_session_marker(content);
    assert_eq!(filtered, content);
}

#[test]
fn normalize_patch_path_simple() {
    let path = Path::new("lib/index.js");
    assert_eq!(normalize_patch_path(path), "lib/index.js");
}

#[test]
fn normalize_patch_path_strips_curdir() {
    let path = Path::new("./lib/index.js");
    assert_eq!(normalize_patch_path(path), "lib/index.js");
}

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}
