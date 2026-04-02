mod manifest;
mod payload;
mod request;

use crate::{Project, Result, SnpmConfig, console};

use std::collections::BTreeSet;
use std::path::Path;

use super::pack::PackInspection;
use manifest::{PackageIdentity, publish_identity, read_manifest_value};
use payload::build_publish_payload;
use request::send_publish_request;

pub struct PublishOptions {
    pub tag: String,
    pub access: Option<String>,
    pub otp: Option<String>,
    pub dry_run: bool,
    pub allow_risks: Vec<String>,
}

impl Default for PublishOptions {
    fn default() -> Self {
        Self {
            tag: "latest".to_string(),
            access: None,
            otp: None,
            dry_run: false,
            allow_risks: Vec::new(),
        }
    }
}

pub async fn publish(
    config: &SnpmConfig,
    project: &Project,
    inspection: &PackInspection,
    tarball_path: &Path,
    options: &PublishOptions,
) -> Result<()> {
    let package = publish_identity(project)?;
    validate_publish(project, &package, inspection, tarball_path, options)?;

    if options.dry_run {
        log_dry_run(&package, options);
        return Ok(());
    }

    let manifest_value = read_manifest_value(project)?;
    let payload = build_publish_payload(config, &package, &manifest_value, tarball_path, options)?;

    console::step(&format!("Publishing {}@{}", package.name, package.version));

    send_publish_request(config, &package, options, payload).await?;

    console::info(&format!(
        "Published {}@{} with tag \"{}\"",
        package.name, package.version, options.tag
    ));

    Ok(())
}

fn log_dry_run(package: &PackageIdentity, options: &PublishOptions) {
    console::info(&format!(
        "Dry run: would publish {}@{} with tag \"{}\"",
        package.name, package.version, options.tag
    ));
}

fn validate_publish(
    project: &Project,
    package: &PackageIdentity,
    inspection: &PackInspection,
    tarball_path: &Path,
    options: &PublishOptions,
) -> Result<()> {
    if project.manifest.private {
        return Err(crate::SnpmError::PublishFailed {
            name: package.name.clone(),
            version: package.version.clone(),
            reason: "package.json has \"private\": true".to_string(),
        });
    }

    let allowed_risks = allowed_risks(project, options);
    let blocked: Vec<_> = inspection
        .findings
        .iter()
        .filter(|finding| finding.is_blocking() && !allowed_risks.contains(&finding.code))
        .collect();

    if blocked.is_empty() {
        return Ok(());
    }

    let reason = blocked
        .iter()
        .map(|finding| match finding.path.as_deref() {
            Some(path) => format!("{} ({path})", finding.code),
            None => finding.code.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");

    Err(crate::SnpmError::PublishFailed {
        name: package.name.clone(),
        version: package.version.clone(),
        reason: format!(
            "publish safety check failed for {}: {}. Re-run with --allow-risk <CODE> only if this is intentional",
            tarball_path.display(),
            reason
        ),
    })
}

fn allowed_risks(project: &Project, options: &PublishOptions) -> BTreeSet<String> {
    let mut allowed = BTreeSet::new();

    if let Some(config) = project
        .manifest
        .snpm
        .as_ref()
        .and_then(|config| config.publish.as_ref())
    {
        allowed.extend(
            config
                .allow_risks
                .iter()
                .map(|risk| risk.to_ascii_uppercase()),
        );
    }

    allowed.extend(
        options
            .allow_risks
            .iter()
            .map(|risk| risk.to_ascii_uppercase()),
    );
    allowed
}

#[cfg(test)]
mod tests {
    use super::{PublishOptions, publish_identity, validate_publish};
    use crate::Project;
    use crate::operations::inspect_pack;

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
    fn validate_publish_rejects_private_packages() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("index.js"), "module.exports = 1;\n").unwrap();
        write_manifest(
            root,
            serde_json::json!({
                "name": "demo",
                "version": "1.0.0",
                "private": true
            }),
        );

        let project = load_project(root);
        let package = publish_identity(&project).unwrap();
        let inspection = inspect_pack(&project).unwrap();
        let error = validate_publish(
            &project,
            &package,
            &inspection,
            &root.join("demo-1.0.0.tgz"),
            &PublishOptions::default(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("\"private\": true"));
    }

    #[test]
    fn validate_publish_blocks_embedded_source_maps_by_default() {
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
  "sourcesContent": ["const secret = 42;"]
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

        let project = load_project(root);
        let package = publish_identity(&project).unwrap();
        let inspection = inspect_pack(&project).unwrap();
        let error = validate_publish(
            &project,
            &package,
            &inspection,
            &root.join("demo-1.0.0.tgz"),
            &PublishOptions::default(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("EMBEDDED_SOURCE_MAP"));
    }

    #[test]
    fn validate_publish_allows_explicit_risk_override() {
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
  "sourcesContent": ["const secret = 42;"]
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

        let project = load_project(root);
        let package = publish_identity(&project).unwrap();
        let inspection = inspect_pack(&project).unwrap();
        let options = PublishOptions {
            allow_risks: vec!["EMBEDDED_SOURCE_MAP".to_string()],
            ..PublishOptions::default()
        };

        validate_publish(
            &project,
            &package,
            &inspection,
            &root.join("demo-1.0.0.tgz"),
            &options,
        )
        .unwrap();
    }

    #[test]
    fn validate_publish_blocks_secret_files() {
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

        let project = load_project(root);
        let package = publish_identity(&project).unwrap();
        let inspection = inspect_pack(&project).unwrap();
        let error = validate_publish(
            &project,
            &package,
            &inspection,
            &root.join("demo-1.0.0.tgz"),
            &PublishOptions::default(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("SECRET_FILE"));
    }

    #[test]
    fn validate_publish_blocks_deny_pattern_matches() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("dist/internal")).unwrap();
        fs::write(
            root.join("dist/internal/debug.js"),
            "console.log('debug');\n",
        )
        .unwrap();
        write_manifest(
            root,
            serde_json::json!({
                "name": "demo",
                "version": "1.0.0",
                "files": ["dist"],
                "snpm": {
                    "publish": {
                        "deny": ["dist/internal/**"]
                    }
                }
            }),
        );

        let project = load_project(root);
        let package = publish_identity(&project).unwrap();
        let inspection = inspect_pack(&project).unwrap();
        let error = validate_publish(
            &project,
            &package,
            &inspection,
            &root.join("demo-1.0.0.tgz"),
            &PublishOptions::default(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("DENY_PATTERN_MATCH"));
    }

    #[test]
    fn validate_publish_blocks_when_max_files_is_exceeded() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("a.js"), "a\n").unwrap();
        fs::write(root.join("b.js"), "b\n").unwrap();
        write_manifest(
            root,
            serde_json::json!({
                "name": "demo",
                "version": "1.0.0",
                "files": ["a.js", "b.js"],
                "snpm": {
                    "publish": {
                        "maxFiles": 1
                    }
                }
            }),
        );

        let project = load_project(root);
        let package = publish_identity(&project).unwrap();
        let inspection = inspect_pack(&project).unwrap();
        let error = validate_publish(
            &project,
            &package,
            &inspection,
            &root.join("demo-1.0.0.tgz"),
            &PublishOptions::default(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("MAX_FILES_EXCEEDED"));
    }

    #[test]
    fn validate_publish_blocks_when_max_bytes_is_exceeded() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("index.js"), "1234567890\n").unwrap();
        write_manifest(
            root,
            serde_json::json!({
                "name": "demo",
                "version": "1.0.0",
                "files": ["index.js"],
                "snpm": {
                    "publish": {
                        "maxBytes": 5
                    }
                }
            }),
        );

        let project = load_project(root);
        let package = publish_identity(&project).unwrap();
        let inspection = inspect_pack(&project).unwrap();
        let error = validate_publish(
            &project,
            &package,
            &inspection,
            &root.join("demo-1.0.0.tgz"),
            &PublishOptions::default(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("MAX_BYTES_EXCEEDED"));
    }

    #[test]
    fn validate_publish_blocks_when_source_maps_are_forbidden() {
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
                "files": ["dist"],
                "snpm": {
                    "publish": {
                        "sourceMaps": "forbid"
                    }
                }
            }),
        );

        let project = load_project(root);
        let package = publish_identity(&project).unwrap();
        let inspection = inspect_pack(&project).unwrap();
        let error = validate_publish(
            &project,
            &package,
            &inspection,
            &root.join("demo-1.0.0.tgz"),
            &PublishOptions::default(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("SOURCE_MAP_PRESENT"));
    }

    #[test]
    fn validate_publish_allows_warning_only_source_maps_when_policy_is_allow() {
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
                "files": ["dist"],
                "snpm": {
                    "publish": {
                        "sourceMaps": "allow"
                    }
                }
            }),
        );

        let project = load_project(root);
        let package = publish_identity(&project).unwrap();
        let inspection = inspect_pack(&project).unwrap();

        validate_publish(
            &project,
            &package,
            &inspection,
            &root.join("demo-1.0.0.tgz"),
            &PublishOptions::default(),
        )
        .unwrap();
    }

    #[test]
    fn validate_publish_uses_manifest_allow_risks_case_insensitively() {
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
  "sourcesContent": ["const secret = 42;"]
}"#,
        )
        .unwrap();
        write_manifest(
            root,
            serde_json::json!({
                "name": "demo",
                "version": "1.0.0",
                "files": ["dist"],
                "snpm": {
                    "publish": {
                        "allowRisks": ["embedded_source_map"]
                    }
                }
            }),
        );

        let project = load_project(root);
        let package = publish_identity(&project).unwrap();
        let inspection = inspect_pack(&project).unwrap();

        validate_publish(
            &project,
            &package,
            &inspection,
            &root.join("demo-1.0.0.tgz"),
            &PublishOptions::default(),
        )
        .unwrap();
    }
}
