use super::manifest::{package_bin, package_name, package_scripts, read_manifest};
use crate::linker::bins::link_known_bins;
use crate::project::BinField;
use crate::{Result, SnpmConfig, SnpmError, Workspace};

use serde_json::Value;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

// Lifecycle scripts run on the *root* project (or workspace child) during install.
// `prepare` runs here because npm/pnpm treat it as part of "preparing the
// installing project itself" — equivalent to `npm pack` followed by `npm install`.
pub(super) const PROJECT_LIFECYCLE_SCRIPT_NAMES: [&str; 4] =
    ["preinstall", "install", "postinstall", "prepare"];

// Lifecycle scripts run on *registry-installed* dependencies. `prepare` is
// intentionally excluded: per the npm spec, `prepare` runs only when packing
// the package or when installing it from a git/local source. Registry tarballs
// are already-packed artifacts, so their `prepare` step must not re-run on the
// consumer machine — many real packages' `prepare` scripts shell out to
// `rollup`/`tsc`/`husky`/etc. that aren't shipped in the published tarball.
pub(super) const DEPENDENCY_LIFECYCLE_SCRIPT_NAMES: [&str; 3] =
    ["preinstall", "install", "postinstall"];

pub fn run_project_scripts(
    _config: &SnpmConfig,
    _workspace: Option<&Workspace>,
    project_root: &Path,
) -> Result<()> {
    let manifest_path = project_root.join("package.json");
    if !manifest_path.exists() {
        return Ok(());
    }

    let value = read_manifest(&manifest_path)?;
    let Some(scripts) = package_scripts(&value) else {
        return Ok(());
    };

    let display_name = package_name(&value)
        .filter(|name| !name.is_empty())
        .unwrap_or("root");
    let bin = package_bin(&value);
    run_present_scripts(
        display_name,
        project_root,
        scripts,
        bin.as_ref(),
        &PROJECT_LIFECYCLE_SCRIPT_NAMES,
    )?;
    Ok(())
}

pub(super) fn run_present_scripts(
    package_name: &str,
    root: &Path,
    scripts: &serde_json::Map<String, Value>,
    bin: Option<&BinField>,
    script_names: &[&str],
) -> Result<usize> {
    let mut ran = 0;
    let self_bin = bin
        .filter(|_| script_names.iter().any(|name| scripts.contains_key(*name)))
        .and_then(|bin| prepare_self_bin_dir(root, package_name, bin));

    let self_bin_dir = self_bin.as_ref().map(|tmp| tmp.path().join(".bin"));
    for script_name in script_names {
        ran += usize::from(run_script_if_present(
            package_name,
            root,
            scripts,
            script_name,
            self_bin_dir.as_deref(),
        )?);
    }

    Ok(ran)
}

fn run_script_if_present(
    package_name: &str,
    root: &Path,
    scripts: &serde_json::Map<String, Value>,
    key: &str,
    self_bin_dir: Option<&Path>,
) -> Result<bool> {
    let cmd = match scripts.get(key) {
        Some(Value::String(cmd)) if !cmd.is_empty() => cmd.clone(),
        _ => return Ok(false),
    };

    let mut command = make_shell_command(&cmd);
    command.current_dir(root);
    let path_value = build_path(root, self_bin_dir, &format!("{package_name}:{key}"))?;
    command.env("PATH", path_value);

    let status = command.status().map_err(|error| SnpmError::ScriptRun {
        name: format!("{package_name}:{key}"),
        reason: error.to_string(),
    })?;

    if !status.success() {
        let code = status.code().unwrap_or(1);
        return Err(SnpmError::ScriptFailed {
            name: format!("{package_name}:{key}"),
            code,
        });
    }

    Ok(true)
}

fn build_path(root: &Path, self_bin_dir: Option<&Path>, script_name: &str) -> Result<OsString> {
    let mut parts: Vec<PathBuf> = Vec::new();

    // Highest priority: the package's own `bin` entries. Mirrors
    // @npmcli/run-script, which materializes the manifest's bin field
    // into a directory prepended to PATH so a script can shell out to
    // its own bins (e.g. @npmcli/template-oss's postinstall calls its
    // own `template-oss-apply` bin).
    if let Some(dir) = self_bin_dir {
        parts.push(dir.to_path_buf());
    }

    if let Some(node_dir) = crate::node::exec::node_bin_dir_for_subprocess(root) {
        parts.push(node_dir);
    }

    // Project-style layout: `<root>/node_modules/.bin/` holds bins for
    // the project's direct deps. Used by the root project + workspace
    // packages.
    parts.push(root.join("node_modules").join(".bin"));

    // Virtual-store layout: when a dep script runs, `root` is the
    // package's own dir at `<vstore>/<pkg>@<ver>/node_modules/<pkg>/`.
    // Sibling deps' bins live one level up at
    // `<vstore>/<pkg>@<ver>/node_modules/.bin/`. Without this entry,
    // postinstalls that call a sibling CLI (e.g. unrs-resolver's
    // postinstall calling `napi-postinstall`) fail with exit 127.
    if let Some(parent) = root.parent() {
        let sibling_bin = parent.join(".bin");
        if sibling_bin != root.join("node_modules").join(".bin") {
            parts.push(sibling_bin);
        }
    }

    if let Some(existing) = env::var_os("PATH") {
        for path in env::split_paths(&existing) {
            parts.push(path);
        }
    }

    env::join_paths(parts).map_err(|error| SnpmError::ScriptRun {
        name: script_name.to_string(),
        reason: error.to_string(),
    })
}

fn prepare_self_bin_dir(pkg_root: &Path, pkg_name: &str, bin: &BinField) -> Option<TempDir> {
    let tmp = TempDir::with_prefix("snpm-self-bin-").ok()?;
    // link_known_bins writes into `<root>/.bin/` (it appends `.bin`
    // itself), so the caller's root is the temp dir's parent of `.bin`.
    if link_known_bins(pkg_root, tmp.path(), pkg_name, bin).is_err() {
        return None;
    }
    let bin_dir = tmp.path().join(".bin");
    if !bin_dir.is_dir() {
        return None;
    }
    Some(tmp)
}

#[cfg(unix)]
fn make_shell_command(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(script);
    command
}

#[cfg(windows)]
fn make_shell_command(script: &str) -> Command {
    let mut command = Command::new("cmd");
    command.arg("/C").arg(script);
    command
}

#[cfg(test)]
mod tests {
    use super::run_present_scripts;

    use serde_json::{Map, Value};
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn write_bin_command(root: &Path, name: &str, marker_name: &str) {
        let bin_dir = root.join("node_modules").join(".bin");
        fs::create_dir_all(&bin_dir).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let script_path = bin_dir.join(name);
            fs::write(
                &script_path,
                format!("#!/bin/sh\necho ok > {marker_name}\n"),
            )
            .unwrap();

            let mut permissions = fs::metadata(&script_path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script_path, permissions).unwrap();
        }

        #[cfg(windows)]
        {
            let script_path = bin_dir.join(format!("{name}.cmd"));
            fs::write(
                &script_path,
                format!("@echo off\r\necho ok > {marker_name}\r\n"),
            )
            .unwrap();
        }
    }

    #[test]
    fn run_present_scripts_uses_local_node_modules_bin() {
        let dir = tempdir().unwrap();
        let marker = dir.path().join("prepare-marker.txt");

        write_bin_command(dir.path(), "effect-language-service", "prepare-marker.txt");

        let mut scripts = Map::new();
        scripts.insert(
            "prepare".to_string(),
            Value::String("effect-language-service".to_string()),
        );

        let ran = run_present_scripts(
            "pkg",
            dir.path(),
            &scripts,
            None,
            &super::PROJECT_LIFECYCLE_SCRIPT_NAMES,
        )
        .unwrap();

        assert!(marker.is_file());
        assert_eq!(ran, 1);
    }

    #[cfg(unix)]
    #[test]
    fn run_present_scripts_exposes_packages_own_bin() {
        use crate::project::BinField;

        let dir = tempdir().unwrap();
        let marker = dir.path().join("self-bin-ran.txt");

        let cli_src = dir.path().join("cli.js");
        fs::write(
            &cli_src,
            format!("#!/bin/sh\necho ok > '{}'\n", marker.display()),
        )
        .unwrap();

        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&cli_src).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&cli_src, permissions).unwrap();

        let bin = BinField::Map(
            [("my-own-tool".to_string(), "cli.js".to_string())]
                .into_iter()
                .collect(),
        );

        let mut scripts = serde_json::Map::new();
        scripts.insert(
            "postinstall".to_string(),
            Value::String("my-own-tool".to_string()),
        );

        let ran = run_present_scripts(
            "self-bin-pkg",
            dir.path(),
            &scripts,
            Some(&bin),
            &super::DEPENDENCY_LIFECYCLE_SCRIPT_NAMES,
        )
        .unwrap();

        assert_eq!(ran, 1);
        assert!(
            marker.is_file(),
            "script should have invoked the package's own bin"
        );
    }
}
