use super::parse::{extract_license, read_license_from_directory};

#[test]
fn extract_license_string() {
    let manifest = serde_json::json!({ "license": "MIT" });
    assert_eq!(extract_license(&manifest), "MIT");
}

#[test]
fn extract_license_object_with_type() {
    let manifest =
        serde_json::json!({ "license": { "type": "ISC", "url": "https://example.com" } });
    assert_eq!(extract_license(&manifest), "ISC");
}

#[test]
fn extract_license_legacy_array() {
    let manifest = serde_json::json!({
        "licenses": [
            { "type": "MIT" },
            { "type": "Apache-2.0" }
        ]
    });
    assert_eq!(extract_license(&manifest), "MIT OR Apache-2.0");
}

#[test]
fn extract_license_unknown() {
    let manifest = serde_json::json!({ "name": "pkg" });
    assert_eq!(extract_license(&manifest), "UNKNOWN");
}

#[test]
fn extract_license_empty_licenses_array() {
    let manifest = serde_json::json!({ "licenses": [] });
    assert_eq!(extract_license(&manifest), "UNKNOWN");
}

#[test]
fn read_license_from_directory_works() {
    let dir = tempfile::tempdir().unwrap();
    let pkg_dir = dir.path().join("my-pkg");
    std::fs::create_dir_all(&pkg_dir).unwrap();
    std::fs::write(
        pkg_dir.join("package.json"),
        r#"{ "name": "my-pkg", "version": "1.0.0", "license": "MIT" }"#,
    )
    .unwrap();

    let entry = read_license_from_directory(&pkg_dir, "fallback").unwrap();
    assert_eq!(entry.name, "my-pkg");
    assert_eq!(entry.version, "1.0.0");
    assert_eq!(entry.license, "MIT");
}

#[test]
fn read_license_from_directory_uses_fallback_name() {
    let dir = tempfile::tempdir().unwrap();
    let pkg_dir = dir.path().join("unnamed");
    std::fs::create_dir_all(&pkg_dir).unwrap();
    std::fs::write(
        pkg_dir.join("package.json"),
        r#"{ "version": "2.0.0", "license": "ISC" }"#,
    )
    .unwrap();

    let entry = read_license_from_directory(&pkg_dir, "fallback-name").unwrap();
    assert_eq!(entry.name, "fallback-name");
}

#[test]
fn read_license_from_directory_returns_none_when_no_manifest() {
    let dir = tempfile::tempdir().unwrap();
    assert!(read_license_from_directory(dir.path(), "test").is_none());
}
