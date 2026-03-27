use crate::linker::fs::copy_dir;
use crate::{Project, Result, SnpmError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PATCHES_DIR: &str = "patches";
const SESSION_MARKER: &str = ".snpm_patch_session";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchSession {
    pub package_name: String,
    pub package_version: String,
    pub original_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PatchInfo {
    pub package_name: String,
    pub package_version: String,
    pub patch_path: PathBuf,
}

pub fn patches_dir(project: &Project) -> PathBuf {
    project.root.join(PATCHES_DIR)
}

pub fn parse_patch_key(key: &str) -> Option<(String, String)> {
    let at_pos = key.rfind('@')?;
    if at_pos == 0 {
        return None;
    }
    Some((key[..at_pos].to_string(), key[at_pos + 1..].to_string()))
}

pub fn find_installed_package(project: &Project, name: &str) -> Result<(String, PathBuf)> {
    let package_path = project.root.join("node_modules").join(name);
    let package_json_path = package_path.join("package.json");

    if !package_json_path.exists() {
        return Err(SnpmError::PackageNotInstalled {
            name: name.to_string(),
            version: "unknown".to_string(),
        });
    }

    let content = fs::read_to_string(&package_json_path).map_err(|source| SnpmError::ReadFile {
        path: package_json_path.clone(),
        source,
    })?;

    let json: serde_json::Value =
        serde_json::from_str(&content).map_err(|source| SnpmError::ParseJson {
            path: package_json_path.clone(),
            source,
        })?;

    let version = json
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SnpmError::ManifestInvalid {
            path: package_json_path,
            reason: "missing version field".to_string(),
        })?
        .to_string();

    Ok((version, package_path))
}

pub fn prepare_patch_directory(
    package_name: &str,
    package_version: &str,
    source_path: &Path,
) -> Result<PathBuf> {
    let safe_name = package_name.replace('/', "+");
    let dir_name = format!("{}@{}", safe_name, package_version);
    let patch_dir = std::env::temp_dir()
        .join(format!("snpm-patch-{}", std::process::id()))
        .join(&dir_name);

    if patch_dir.exists() {
        fs::remove_dir_all(&patch_dir).map_err(|source| SnpmError::Io {
            path: patch_dir.clone(),
            source,
        })?;
    }

    copy_directory(source_path, &patch_dir)?;

    let session = PatchSession {
        package_name: package_name.to_string(),
        package_version: package_version.to_string(),
        original_path: source_path.to_path_buf(),
    };

    let session_path = patch_dir.join(SESSION_MARKER);
    let session_json =
        serde_json::to_string_pretty(&session).map_err(|e| SnpmError::SerializeJson {
            path: session_path.clone(),
            reason: e.to_string(),
        })?;

    fs::write(&session_path, session_json).map_err(|source| SnpmError::WriteFile {
        path: session_path,
        source,
    })?;

    Ok(patch_dir)
}

pub fn read_patch_session(patch_dir: &Path) -> Result<PatchSession> {
    let session_path = patch_dir.join(SESSION_MARKER);

    if !session_path.exists() {
        return Err(SnpmError::PatchSessionNotFound {
            path: patch_dir.to_path_buf(),
        });
    }

    let content = fs::read_to_string(&session_path).map_err(|source| SnpmError::ReadFile {
        path: session_path.clone(),
        source,
    })?;

    serde_json::from_str(&content).map_err(|source| SnpmError::ParseJson {
        path: session_path,
        source,
    })
}

pub fn create_patch(
    project: &Project,
    session: &PatchSession,
    modified_dir: &Path,
) -> Result<PathBuf> {
    let patches = patches_dir(project);
    fs::create_dir_all(&patches).map_err(|source| SnpmError::WriteFile {
        path: patches.clone(),
        source,
    })?;

    let safe_name = session.package_name.replace('/', "+");
    let patch_filename = format!("{}@{}.patch", safe_name, session.package_version);
    let patch_path = patches.join(&patch_filename);

    let diff_output = run_diff(&session.original_path, modified_dir, &session.package_name)?;

    if diff_output.trim().is_empty() {
        return Err(SnpmError::PatchCreate {
            name: session.package_name.clone(),
            reason: "no changes detected".to_string(),
        });
    }

    let filtered = filter_session_marker(&diff_output);

    fs::write(&patch_path, filtered).map_err(|source| SnpmError::WriteFile {
        path: patch_path.clone(),
        source,
    })?;

    Ok(patch_path)
}

fn run_diff(original: &Path, modified: &Path, package_name: &str) -> Result<String> {
    let original = original.canonicalize().map_err(|source| SnpmError::Io {
        path: original.to_path_buf(),
        source,
    })?;
    let modified = modified.canonicalize().map_err(|source| SnpmError::Io {
        path: modified.to_path_buf(),
        source,
    })?;

    let output = Command::new("diff")
        .args(["-ruN"])
        .arg(&original)
        .arg(&modified)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|_| SnpmError::PatchCreate {
            name: package_name.to_string(),
            reason: "diff command not found - please install diffutils".to_string(),
        })?;

    match output.status.code() {
        Some(0 | 1) => Ok(rewrite_diff_paths(
            &String::from_utf8_lossy(&output.stdout),
            &original,
            &modified,
        )),
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(SnpmError::PatchCreate {
                name: package_name.to_string(),
                reason: if stderr.is_empty() {
                    "diff command failed".to_string()
                } else {
                    stderr
                },
            })
        }
    }
}

fn rewrite_diff_paths(content: &str, original_root: &Path, modified_root: &Path) -> String {
    let mut rewritten = String::new();

    for line in content.lines() {
        let line = rewrite_patch_header(line, "--- ", original_root, "a")
            .or_else(|| rewrite_patch_header(line, "+++ ", modified_root, "b"))
            .unwrap_or_else(|| line.to_string());
        rewritten.push_str(&line);
        rewritten.push('\n');
    }

    rewritten
}

fn rewrite_patch_header(line: &str, marker: &str, root: &Path, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(marker)?;
    if rest == "/dev/null" || rest.starts_with("/dev/null\t") {
        return Some(line.to_string());
    }

    let (path_text, suffix) = match rest.split_once('\t') {
        Some((path_text, suffix)) => (path_text, Some(suffix)),
        None => (rest, None),
    };

    let relative = Path::new(path_text).strip_prefix(root).ok()?;
    let mut rewritten = format!("{marker}{prefix}/{}", normalize_patch_path(relative));

    if let Some(suffix) = suffix {
        rewritten.push('\t');
        rewritten.push_str(suffix);
    }

    Some(rewritten)
}

fn normalize_patch_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn filter_session_marker(content: &str) -> String {
    let mut result = String::new();
    let mut skip_until_next_diff = false;

    for line in content.lines() {
        if line.starts_with("diff ") {
            skip_until_next_diff = line.contains(SESSION_MARKER);
        }

        if !skip_until_next_diff {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

pub fn apply_patch(target_dir: &Path, patch_path: &Path) -> Result<()> {
    if !patch_path.exists() {
        return Err(SnpmError::PatchNotFound {
            name: patch_path.to_string_lossy().to_string(),
            reason: "patch file does not exist".to_string(),
        });
    }

    let current_dir = target_dir.canonicalize().map_err(|source| SnpmError::Io {
        path: target_dir.to_path_buf(),
        source,
    })?;

    let output = Command::new("patch")
        .args(["-p1", "-i"])
        .arg(patch_path)
        .current_dir(&current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|_| SnpmError::PatchApply {
            name: patch_path.to_string_lossy().to_string(),
            reason: "patch command not found - please install patch".to_string(),
        })?;

    if output.status.success() {
        Ok(())
    } else {
        Err(SnpmError::PatchApply {
            name: patch_path.to_string_lossy().to_string(),
            reason: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

pub fn materialize_patch_target(target_dir: &Path, store_path: &Path) -> Result<()> {
    if let Ok(meta) = target_dir.symlink_metadata() {
        if meta.is_dir() && !meta.file_type().is_symlink() {
            fs::remove_dir_all(target_dir).map_err(|source| SnpmError::Io {
                path: target_dir.to_path_buf(),
                source,
            })?;
        } else {
            fs::remove_file(target_dir).map_err(|source| SnpmError::Io {
                path: target_dir.to_path_buf(),
                source,
            })?;
        }
    }

    crate::linker::fs::ensure_parent_dir(target_dir)?;

    copy_dir(store_path, target_dir)
}

pub fn remove_patch(project: &Project, package_name: &str) -> Result<Option<PathBuf>> {
    let patches = patches_dir(project);
    if !patches.exists() {
        return Ok(None);
    }

    let safe_name = package_name.replace('/', "+");
    let prefix = format!("{}@", safe_name);

    for entry in fs::read_dir(&patches)
        .map_err(|source| SnpmError::ReadFile {
            path: patches.clone(),
            source,
        })?
        .flatten()
    {
        let path = entry.path();
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };

        if filename.starts_with(&prefix) && filename.ends_with(".patch") {
            fs::remove_file(&path).map_err(|source| SnpmError::Io {
                path: path.clone(),
                source,
            })?;
            return Ok(Some(path));
        }
    }

    Ok(None)
}

pub fn list_patches(project: &Project) -> Result<Vec<PatchInfo>> {
    let patches = patches_dir(project);
    if !patches.exists() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();

    for entry in fs::read_dir(&patches)
        .map_err(|source| SnpmError::ReadFile {
            path: patches.clone(),
            source,
        })?
        .flatten()
    {
        let path = entry.path();
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) if name.ends_with(".patch") => name,
            _ => continue,
        };

        let name_version = match filename.strip_suffix(".patch") {
            Some(nv) => nv,
            None => continue,
        };

        if let Some((name, version)) = parse_patch_key(name_version) {
            result.push(PatchInfo {
                package_name: name.replace('+', "/"),
                package_version: version,
                patch_path: path,
            });
        }
    }

    Ok(result)
}

pub fn get_patched_dependencies(project: &Project) -> BTreeMap<String, String> {
    let mut patched = BTreeMap::new();

    if let Some(ref pnpm) = project.manifest.pnpm
        && let Some(ref deps) = pnpm.patched_dependencies
    {
        patched.extend(deps.clone());
    }

    if let Some(ref snpm) = project.manifest.snpm
        && let Some(ref deps) = snpm.patched_dependencies
    {
        patched.extend(deps.clone());
    }

    patched
}

pub fn cleanup_patch_session(patch_dir: &Path) -> Result<()> {
    if patch_dir.exists() {
        fs::remove_dir_all(patch_dir).map_err(|source| SnpmError::Io {
            path: patch_dir.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

fn copy_directory(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).map_err(|source| SnpmError::WriteFile {
        path: dst.to_path_buf(),
        source,
    })?;

    for entry in fs::read_dir(src)
        .map_err(|source| SnpmError::ReadFile {
            path: src.to_path_buf(),
            source,
        })?
        .flatten()
    {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str == "node_modules" || name_str == ".git" {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst.join(&name);

        if src_path.is_dir() {
            copy_directory(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(|source| SnpmError::Io {
                path: dst_path,
                source,
            })?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{apply_patch, materialize_patch_target, rewrite_diff_paths, run_diff};
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

        apply_patch(&target, &patch_path).expect("apply patch");

        let patched = fs::read_to_string(target.join("lib/index.js")).expect("read target");
        assert_eq!(patched, "module.exports = 'new';\n");
    }

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

    fn command_available(command: &str) -> bool {
        Command::new(command)
            .arg("--help")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
    }
}
