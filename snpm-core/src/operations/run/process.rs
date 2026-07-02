use crate::{Result, SnpmError};

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Join extra script args into the string appended to the `sh -c` /
/// `cmd /C` command line. Each arg is shell-quoted: without quoting,
/// `snpm run lint -- "src/my file.ts"` would re-split into two args and
/// anything containing `;`, `$(...)`, or `&&` would be executed by the
/// shell.
pub(in crate::operations::run) fn join_args(args: &[String]) -> String {
    args.iter()
        .map(|arg| quote_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(unix)]
fn quote_arg(arg: &str) -> String {
    let safe = !arg.is_empty()
        && arg.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '_' | '-' | '.' | '/' | ':' | '=' | '@' | '%' | ',' | '+')
        });
    if safe {
        return arg.to_string();
    }
    // Single quotes disable all shell interpretation; embedded single
    // quotes become '\'' (close, escaped quote, reopen).
    format!("'{}'", arg.replace('\'', "'\\''"))
}

#[cfg(windows)]
fn quote_arg(arg: &str) -> String {
    let safe = !arg.is_empty()
        && arg.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    '_' | '-' | '.' | '/' | ':' | '=' | '@' | '%' | ',' | '+' | '\\'
                )
        });
    if safe {
        return arg.to_string();
    }
    format!("\"{}\"", arg.replace('"', "\"\""))
}

pub(in crate::operations::run) fn build_path(
    bin_dir: PathBuf,
    script: &str,
    project_root: &Path,
    node_bin_override: Option<&Path>,
) -> Result<OsString> {
    let mut parts = Vec::new();

    // An explicit override (e.g. `snpm node run`) wins; otherwise fall
    // back to the project's pinned/active Node discovery. Threading the
    // override as a parameter instead of mutating the process env keeps
    // this safe under the multi-threaded runtime.
    match node_bin_override {
        Some(node_dir) => parts.push(node_dir.to_path_buf()),
        None => {
            if let Some(node_dir) = crate::node::exec::node_bin_dir_for_subprocess(project_root) {
                parts.push(node_dir);
            }
        }
    }

    parts.push(bin_dir);

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

    #[cfg(unix)]
    #[test]
    fn join_args_quotes_spaces_and_metacharacters() {
        assert_eq!(
            join_args(&["src/my file.ts".to_string()]),
            "'src/my file.ts'"
        );
        assert_eq!(join_args(&["a;rm -rf x".to_string()]), "'a;rm -rf x'");
        assert_eq!(join_args(&["$(whoami)".to_string()]), "'$(whoami)'");
        assert_eq!(join_args(&["it's".to_string()]), "'it'\\''s'");
        assert_eq!(join_args(&[String::new()]), "''");
    }

    #[cfg(unix)]
    #[test]
    fn join_args_leaves_plain_flags_untouched() {
        assert_eq!(
            join_args(&["--watch".to_string(), "--grep=foo".to_string()]),
            "--watch --grep=foo"
        );
    }
}
