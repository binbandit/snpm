use crate::{Result, SnpmError};

use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

pub(in crate::operations::run) fn join_args(args: &[String]) -> String {
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

pub(in crate::operations::run) fn build_path(bin_dir: PathBuf, script: &str) -> Result<OsString> {
    let mut parts = vec![bin_dir];

    if let Some(existing) = env::var_os("PATH") {
        for path in env::split_paths(&existing) {
            parts.push(path);
        }
    }

    env::join_paths(parts).map_err(|error| SnpmError::ScriptRun {
        name: script.to_string(),
        reason: error.to_string(),
    })
}

pub(in crate::operations::run) fn make_direct_command(command: &str, args: &[String]) -> Command {
    let mut process = Command::new(command);
    process.args(args);
    process
}

#[cfg(unix)]
pub(in crate::operations::run) fn make_command(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(script);
    command
}

#[cfg(windows)]
pub(in crate::operations::run) fn make_command(script: &str) -> Command {
    let mut command = Command::new("cmd");
    command.arg("/C").arg(script);
    command
}

#[cfg(test)]
mod tests {
    use super::join_args;

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
