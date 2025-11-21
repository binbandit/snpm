use crate::{Project, Result, SnpmError};
use std::env;
use std::path::PathBuf;
use std::process::Command;

pub fn run_script(project: &Project, script: &str, args: &[String]) -> Result<()> {
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
