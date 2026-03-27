use crate::{Project, Result, SnpmError, Workspace, console};
use std::env;
use std::path::PathBuf;
use std::process::Command;

pub fn run_script(project: &Project, script: &str, args: &[String]) -> Result<()> {
    let scripts = &project.manifest.scripts;

    if !scripts.contains_key(script) {
        return Err(SnpmError::ScriptMissing {
            name: script.to_string(),
        });
    }

    let pre_name = format!("pre{}", script);
    if scripts.contains_key(&pre_name) {
        run_single_script(project, &pre_name, &[])?;
    }

    run_single_script(project, script, args)?;

    let post_name = format!("post{}", script);
    if scripts.contains_key(&post_name) {
        run_single_script(project, &post_name, &[])?;
    }

    Ok(())
}

pub fn run_workspace_scripts(
    workspace: &Workspace,
    script: &str,
    filters: &[String],
    args: &[String],
) -> Result<()> {
    let mut any_ran = false;

    for project in &workspace.projects {
        let name = project_label(project);

        if !matches_filters(&name, filters) {
            continue;
        }

        if !project.manifest.scripts.contains_key(script) {
            continue;
        }

        any_ran = true;
        println!("\n{}", name);
        run_script(project, script, args)?;
    }

    if !any_ran {
        return Err(SnpmError::ScriptMissing {
            name: script.to_string(),
        });
    }

    Ok(())
}

fn run_single_script(project: &Project, script: &str, args: &[String]) -> Result<()> {
    let scripts = &project.manifest.scripts;
    let base = scripts
        .get(script)
        .ok_or_else(|| SnpmError::ScriptMissing {
            name: script.to_string(),
        })?;

    let mut command_text = base.clone();

    if !args.is_empty() {
        let extra = join_args(args);
        if !command_text.is_empty() {
            command_text.push(' ');
        }
        command_text.push_str(&extra);
    }

    console::info(&command_text);

    let mut command = make_command(&command_text);

    command.current_dir(&project.root);

    let bin_dir = project.root.join("node_modules").join(".bin");
    let path_value = build_path(bin_dir, script)?;
    command.env("PATH", path_value);

    let status = command.status().map_err(|error| SnpmError::ScriptRun {
        name: script.to_string(),
        reason: error.to_string(),
    })?;

    if status.success() {
        Ok(())
    } else {
        let code = status.code().unwrap_or(1);
        Err(SnpmError::ScriptFailed {
            name: script.to_string(),
            code,
        })
    }
}

fn join_args(args: &[String]) -> String {
    let mut result = String::new();
    let mut first = true;

    for arg in args {
        if !first {
            result.push(' ');
        }
        first = false;
        result.push_str(arg);
    }

    result
}

fn build_path(bin_dir: PathBuf, script: &str) -> Result<std::ffi::OsString> {
    let mut parts = Vec::new();
    parts.push(bin_dir);

    if let Some(existing) = env::var_os("PATH") {
        for path in env::split_paths(&existing) {
            parts.push(path);
        }
    }

    let joined = env::join_paths(parts).map_err(|error| SnpmError::ScriptRun {
        name: script.to_string(),
        reason: error.to_string(),
    })?;

    Ok(joined)
}

fn matches_filters(name: &str, filters: &[String]) -> bool {
    if filters.is_empty() {
        return true;
    }

    for filter in filters {
        if filter == name {
            return true;
        }

        if let Ok(pattern) = glob::Pattern::new(filter) {
            if pattern.matches(name) {
                return true;
            }
        } else if name.contains(filter) {
            return true;
        }
    }

    false
}

fn project_label(project: &Project) -> String {
    if let Some(name) = project.manifest.name.as_deref() {
        name.to_string()
    } else {
        project
            .root
            .file_name()
            .and_then(|os| os.to_str())
            .unwrap_or(".")
            .to_string()
    }
}

pub struct ExecOptions<'a> {
    pub command: &'a str,
    pub args: &'a [String],
    pub shell_mode: bool,
}

pub fn exec_command(project: &Project, options: &ExecOptions) -> Result<()> {
    let bin_dir = project.root.join("node_modules").join(".bin");
    let path_value = build_path(bin_dir, options.command)?;

    let package_name = project.manifest.name.as_deref().unwrap_or_default();

    let full_command = if options.args.is_empty() {
        options.command.to_string()
    } else {
        format!("{} {}", options.command, join_args(options.args))
    };

    console::info(&full_command);

    let mut process = if options.shell_mode {
        make_command(&full_command)
    } else {
        make_direct_command(options.command, options.args)
    };

    process.current_dir(&project.root);
    process.env("PATH", path_value);
    process.env("SNPM_PACKAGE_NAME", package_name);

    let status = process.status().map_err(|error| SnpmError::ScriptRun {
        name: options.command.to_string(),
        reason: error.to_string(),
    })?;

    if status.success() {
        Ok(())
    } else {
        Err(SnpmError::ScriptFailed {
            name: options.command.to_string(),
            code: status.code().unwrap_or(1),
        })
    }
}

pub fn exec_workspace_command(
    workspace: &Workspace,
    options: &ExecOptions,
    filters: &[String],
) -> Result<()> {
    for project in &workspace.projects {
        let name = project_label(project);

        if !matches_filters(&name, filters) {
            continue;
        }

        println!("\n{}", name);
        exec_command(project, options)?;
    }

    Ok(())
}

fn make_direct_command(command: &str, args: &[String]) -> Command {
    let mut process = Command::new(command);
    process.args(args);
    process
}

#[cfg(unix)]
fn make_command(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(script);
    command
}

#[cfg(windows)]
fn make_command(script: &str) -> Command {
    let mut command = Command::new("cmd");
    command.arg("/C").arg(script);
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Manifest;
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    #[test]
    fn matches_filters_empty_returns_true() {
        assert!(matches_filters("anything", &[]));
    }

    #[test]
    fn matches_filters_exact_match() {
        let filters = vec!["my-pkg".to_string()];
        assert!(matches_filters("my-pkg", &filters));
        assert!(!matches_filters("other-pkg", &filters));
    }

    #[test]
    fn matches_filters_glob_match() {
        let filters = vec!["@scope/*".to_string()];
        assert!(matches_filters("@scope/foo", &filters));
        assert!(!matches_filters("other", &filters));
    }

    #[test]
    fn matches_filters_substring_fallback() {
        // When glob pattern is invalid, falls back to substring contains check
        let filters = vec!["[invalid".to_string()];
        assert!(matches_filters("contains-[invalid-here", &filters));
        assert!(!matches_filters("no-match", &filters));
    }

    #[test]
    fn matches_filters_no_substring_when_glob_valid() {
        // "foo" is a valid glob, so it does NOT do substring matching
        let filters = vec!["foo".to_string()];
        assert!(matches_filters("foo", &filters)); // exact glob match
        assert!(!matches_filters("my-foo-pkg", &filters)); // no substring fallback
    }

    #[test]
    fn matches_filters_multiple() {
        let filters = vec!["pkg-a".to_string(), "pkg-b".to_string()];
        assert!(matches_filters("pkg-a", &filters));
        assert!(matches_filters("pkg-b", &filters));
        assert!(!matches_filters("pkg-c", &filters));
    }

    #[test]
    fn project_label_uses_name() {
        let dir = tempdir().unwrap();
        let project = Project {
            root: dir.path().to_path_buf(),
            manifest_path: dir.path().join("package.json"),
            manifest: Manifest {
                name: Some("my-project".to_string()),
                version: None,
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
                optional_dependencies: BTreeMap::new(),
                scripts: BTreeMap::new(),
                files: None,
                bin: None,
                main: None,
                pnpm: None,
                snpm: None,
                workspaces: None,
            },
        };
        assert_eq!(project_label(&project), "my-project");
    }

    #[test]
    fn project_label_falls_back_to_dir_name() {
        let dir = tempdir().unwrap();
        let project = Project {
            root: dir.path().to_path_buf(),
            manifest_path: dir.path().join("package.json"),
            manifest: Manifest {
                name: None,
                version: None,
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
                optional_dependencies: BTreeMap::new(),
                scripts: BTreeMap::new(),
                files: None,
                bin: None,
                main: None,
                pnpm: None,
                snpm: None,
                workspaces: None,
            },
        };
        let label = project_label(&project);
        assert!(!label.is_empty());
    }

    #[test]
    fn join_args_empty() {
        assert_eq!(join_args(&[]), "");
    }

    #[test]
    fn join_args_single() {
        assert_eq!(join_args(&["hello".to_string()]), "hello");
    }

    #[test]
    fn join_args_multiple() {
        assert_eq!(
            join_args(&["a".to_string(), "b".to_string(), "c".to_string()]),
            "a b c"
        );
    }
}
