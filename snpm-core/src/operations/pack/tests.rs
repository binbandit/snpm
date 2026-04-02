use super::{PackFileReason, inspect_pack};
use crate::Project;

use std::fs;

use tempfile::tempdir;

fn write_manifest(root: &std::path::Path, manifest: serde_json::Value) {
    fs::write(
        root.join("package.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

fn load_project(root: &std::path::Path) -> Project {
    Project::from_manifest_path(root.join("package.json")).unwrap()
}

#[test]
fn inspect_pack_includes_bin_when_files_field_is_present() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::create_dir_all(root.join("dist")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(root.join("dist/index.js"), "export {};\n").unwrap();
    fs::write(root.join("bin/cli.js"), "#!/usr/bin/env node\n").unwrap();
    write_manifest(
        root,
        serde_json::json!({
            "name": "demo",
            "version": "1.0.0",
            "files": ["dist"],
            "bin": "./bin/cli.js"
        }),
    );

    let inspection = inspect_pack(&load_project(root)).unwrap();

    assert!(
        inspection
            .files
            .iter()
            .any(|file| file.path == "bin/cli.js" && file.reason == PackFileReason::BinEntry)
    );
    assert!(
        inspection.files.iter().any(
            |file| file.path == "dist/index.js" && file.reason == PackFileReason::ManifestFiles
        )
    );
}

#[test]
fn inspect_pack_ignores_root_ignore_for_files_but_honors_nested_ignore() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::create_dir_all(root.join("dist")).unwrap();
    fs::write(root.join(".npmignore"), "dist/index.js\n").unwrap();
    fs::write(root.join("dist/.npmignore"), "secret.js\n").unwrap();
    fs::write(root.join("dist/index.js"), "export const ok = true;\n").unwrap();
    fs::write(root.join("dist/secret.js"), "export const nope = true;\n").unwrap();
    write_manifest(
        root,
        serde_json::json!({
            "name": "demo",
            "version": "1.0.0",
            "files": ["dist"]
        }),
    );

    let inspection = inspect_pack(&load_project(root)).unwrap();

    assert!(
        inspection
            .files
            .iter()
            .any(|file| file.path == "dist/index.js")
    );
    assert!(
        !inspection
            .files
            .iter()
            .any(|file| file.path == "dist/secret.js")
    );
}

#[test]
fn inspect_pack_flags_embedded_source_maps() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::create_dir_all(root.join("dist")).unwrap();
    fs::write(root.join("dist/app.js"), "console.log('hi');\n").unwrap();
    fs::write(
        root.join("dist/app.js.map"),
        r#"{
  "version": 3,
  "file": "app.js",
  "sources": ["src/app.ts"],
  "sourcesContent": ["export const secret = 42;"]
}"#,
    )
    .unwrap();
    write_manifest(
        root,
        serde_json::json!({
            "name": "demo",
            "version": "1.0.0",
            "files": ["dist"]
        }),
    );

    let inspection = inspect_pack(&load_project(root)).unwrap();

    assert!(
        inspection
            .findings
            .iter()
            .any(|finding| finding.code == "EMBEDDED_SOURCE_MAP" && finding.is_blocking())
    );
}

#[test]
fn inspect_pack_skips_explicit_file_match_when_child_ignore_blocks_it() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::create_dir_all(root.join("dist")).unwrap();
    fs::write(root.join("dist/.npmignore"), "secret.js\n").unwrap();
    fs::write(root.join("dist/secret.js"), "export const nope = true;\n").unwrap();
    write_manifest(
        root,
        serde_json::json!({
            "name": "demo",
            "version": "1.0.0",
            "files": ["dist/secret.js"]
        }),
    );

    let inspection = inspect_pack(&load_project(root)).unwrap();

    assert!(
        !inspection
            .files
            .iter()
            .any(|file| file.path == "dist/secret.js")
    );
}

#[test]
fn inspect_pack_prefers_npmignore_over_gitignore_during_default_scan() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::create_dir_all(root.join("dist")).unwrap();
    fs::write(root.join(".gitignore"), "dist\n").unwrap();
    fs::write(root.join(".npmignore"), "").unwrap();
    fs::write(root.join("dist/index.js"), "export const ok = true;\n").unwrap();
    write_manifest(
        root,
        serde_json::json!({
            "name": "demo",
            "version": "1.0.0"
        }),
    );

    let inspection = inspect_pack(&load_project(root)).unwrap();

    assert!(
        inspection
            .files
            .iter()
            .any(|file| file.path == "dist/index.js" && file.reason == PackFileReason::DefaultScan)
    );
}

#[test]
fn inspect_pack_includes_mandatory_files_even_when_files_field_omits_them() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::create_dir_all(root.join("dist")).unwrap();
    fs::write(root.join("dist/index.js"), "export {};\n").unwrap();
    fs::write(root.join("README.md"), "# demo\n").unwrap();
    write_manifest(
        root,
        serde_json::json!({
            "name": "demo",
            "version": "1.0.0",
            "files": ["dist"]
        }),
    );

    let inspection = inspect_pack(&load_project(root)).unwrap();

    assert!(
        inspection
            .files
            .iter()
            .any(|file| file.path == "README.md" && file.reason == PackFileReason::Mandatory)
    );
}

#[test]
fn inspect_pack_flags_secret_like_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join(".env.production"), "TOKEN=super-secret\n").unwrap();
    write_manifest(
        root,
        serde_json::json!({
            "name": "demo",
            "version": "1.0.0",
            "files": [".env.production"]
        }),
    );

    let inspection = inspect_pack(&load_project(root)).unwrap();

    assert!(
        inspection
            .findings
            .iter()
            .any(|finding| finding.code == "SECRET_FILE"
                && finding.path.as_deref() == Some(".env.production"))
    );
}

#[test]
fn inspect_pack_warns_on_external_source_maps_by_default() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::create_dir_all(root.join("dist")).unwrap();
    fs::write(root.join("dist/app.js"), "console.log('hi');\n").unwrap();
    fs::write(
        root.join("dist/app.js.map"),
        r#"{
  "version": 3,
  "file": "app.js",
  "sources": ["src/app.ts"]
}"#,
    )
    .unwrap();
    write_manifest(
        root,
        serde_json::json!({
            "name": "demo",
            "version": "1.0.0",
            "files": ["dist"]
        }),
    );

    let inspection = inspect_pack(&load_project(root)).unwrap();

    assert!(
        inspection
            .findings
            .iter()
            .any(|finding| finding.code == "SOURCE_MAP_PRESENT" && !finding.is_blocking())
    );
}
