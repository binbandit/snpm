use crate::project::{ManifestSnpmPublish, SourceMapPolicy};
use crate::{Project, Result, SnpmError};

use std::path::Path;

use super::collect::CollectedFile;
use super::{PackFinding, PackFindingSeverity};

pub(super) fn audit_pack(project: &Project, files: &[CollectedFile]) -> Result<Vec<PackFinding>> {
    let policy = project
        .manifest
        .snpm
        .as_ref()
        .and_then(|config| config.publish.as_ref());
    let source_map_policy = policy
        .and_then(|config| config.source_maps)
        .unwrap_or(SourceMapPolicy::ExternalOnly);
    let mut findings = Vec::new();

    for file in files {
        let path = file.relative_path.as_str();
        let file_name = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path);

        if matches_deny_pattern(path, policy) {
            findings.push(error(
                "DENY_PATTERN_MATCH",
                Some(path),
                format!("matches snpm.publish.deny: {path}"),
            ));
        }

        if is_secret_like_path(file_name) || contains_private_key(&file.absolute_path)? {
            findings.push(error(
                "SECRET_FILE",
                Some(path),
                "looks like a secret, credential, or private key".to_string(),
            ));
        }

        if path.ends_with(".map") {
            audit_source_map(file, source_map_policy, &mut findings);
        } else if looks_suspicious_runtime_excess(path) {
            findings.push(warning(
                "NON_RUNTIME_FILE",
                Some(path),
                "looks like a test, fixture, example, or development-only asset".to_string(),
            ));
        }
    }

    if let Some(max_files) = policy.and_then(|config| config.max_files) {
        if files.len() > max_files {
            findings.push(error(
                "MAX_FILES_EXCEEDED",
                None,
                format!(
                    "package has {} files, exceeding the configured maxFiles of {max_files}",
                    files.len()
                ),
            ));
        }
    }

    let total_bytes: u64 = files.iter().map(|file| file.size).sum();
    if let Some(max_bytes) = policy.and_then(|config| config.max_bytes) {
        if total_bytes > max_bytes {
            findings.push(error(
                "MAX_BYTES_EXCEEDED",
                None,
                format!(
                    "package is {total_bytes} bytes unpacked, exceeding the configured maxBytes of {max_bytes}"
                ),
            ));
        }
    }

    Ok(findings)
}

fn audit_source_map(
    file: &CollectedFile,
    policy: SourceMapPolicy,
    findings: &mut Vec<PackFinding>,
) {
    let path = file.relative_path.as_str();

    if policy == SourceMapPolicy::Forbid {
        findings.push(error(
            "SOURCE_MAP_PRESENT",
            Some(path),
            "source maps are forbidden by snpm.publish.sourceMaps".to_string(),
        ));
        return;
    }

    let content = match std::fs::read_to_string(&file.absolute_path) {
        Ok(content) => content,
        Err(_) => {
            findings.push(error(
                "SOURCE_MAP_UNREADABLE",
                Some(path),
                "source map could not be read and cannot be verified as safe".to_string(),
            ));
            return;
        }
    };
    let value: serde_json::Value = match serde_json::from_str(&content) {
        Ok(value) => value,
        Err(_) => {
            findings.push(error(
                "SOURCE_MAP_INVALID",
                Some(path),
                "source map is invalid JSON and cannot be verified as safe".to_string(),
            ));
            return;
        }
    };
    let has_embedded_sources = value
        .get("sourcesContent")
        .and_then(|sources| sources.as_array())
        .is_some_and(|sources| {
            sources.iter().any(|source| {
                source
                    .as_str()
                    .map(|source| !source.trim().is_empty())
                    .unwrap_or(false)
            })
        });

    if has_embedded_sources {
        findings.push(error(
            "EMBEDDED_SOURCE_MAP",
            Some(path),
            "contains embedded sourcesContent and can reconstruct original source files"
                .to_string(),
        ));
    } else {
        findings.push(warning(
            "SOURCE_MAP_PRESENT",
            Some(path),
            "publishes a source map without embedded sourcesContent".to_string(),
        ));
    }
}

fn matches_deny_pattern(path: &str, policy: Option<&ManifestSnpmPublish>) -> bool {
    let Some(policy) = policy else {
        return false;
    };

    policy
        .deny
        .iter()
        .filter_map(|pattern| glob::Pattern::new(pattern).ok())
        .any(|pattern| pattern.matches(path))
}

fn contains_private_key(path: &Path) -> Result<bool> {
    let content = match std::fs::read(path) {
        Ok(content) => content,
        Err(source) => {
            return Err(SnpmError::ReadFile {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    if content.len() > 128 * 1024 {
        return Ok(false);
    }

    let content = String::from_utf8_lossy(&content);
    Ok(content.contains("-----BEGIN PRIVATE KEY-----")
        || content.contains("-----BEGIN RSA PRIVATE KEY-----")
        || content.contains("-----BEGIN OPENSSH PRIVATE KEY-----"))
}

fn is_secret_like_path(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    if lower == ".env" || (lower.starts_with(".env.") && !is_safe_env_example(&lower)) {
        return true;
    }

    matches!(
        lower.as_str(),
        ".npmrc" | ".pypirc" | "id_rsa" | "id_dsa" | "id_ed25519" | "credentials" | "known_hosts"
    ) || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".p12")
        || lower.ends_with(".pfx")
        || lower.ends_with(".jks")
        || lower.ends_with(".kdbx")
}

fn is_safe_env_example(name: &str) -> bool {
    name.ends_with(".example") || name.ends_with(".sample") || name.ends_with(".template")
}

fn looks_suspicious_runtime_excess(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    starts_with_segment(&lower, "test/")
        || starts_with_segment(&lower, "tests/")
        || starts_with_segment(&lower, "fixture/")
        || starts_with_segment(&lower, "fixtures/")
        || starts_with_segment(&lower, "example/")
        || starts_with_segment(&lower, "examples/")
        || lower.contains("/test/")
        || lower.contains("/tests/")
        || lower.contains("/fixture/")
        || lower.contains("/fixtures/")
        || lower.contains("/example/")
        || lower.contains("/examples/")
        || lower.contains("/__tests__/")
}

fn starts_with_segment(path: &str, prefix: &str) -> bool {
    path.starts_with(prefix) || path.starts_with(&format!("./{prefix}"))
}

fn error(code: &str, path: Option<&str>, message: String) -> PackFinding {
    finding(PackFindingSeverity::Error, code, path, message)
}

fn warning(code: &str, path: Option<&str>, message: String) -> PackFinding {
    finding(PackFindingSeverity::Warning, code, path, message)
}

fn finding(
    severity: PackFindingSeverity,
    code: &str,
    path: Option<&str>,
    message: String,
) -> PackFinding {
    PackFinding {
        code: code.to_string(),
        severity,
        path: path.map(str::to_string),
        message,
    }
}
