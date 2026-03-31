use crate::{Result, SnpmError, console};

use std::path::Path;
use std::process::Command;

pub(super) fn run_bin(bin_path: &Path, arguments: Vec<String>) -> Result<()> {
    console::step(&format!("Running {}", bin_path.display()));

    let mut command = Command::new(bin_path);
    command.args(arguments);
    command.stdin(std::process::Stdio::inherit());
    command.stdout(std::process::Stdio::inherit());
    command.stderr(std::process::Stdio::inherit());

    let status = command.status().map_err(|error| SnpmError::ScriptRun {
        name: bin_path.to_string_lossy().to_string(),
        reason: error.to_string(),
    })?;

    if !status.success() {
        return Err(SnpmError::ScriptFailed {
            name: bin_path.to_string_lossy().to_string(),
            code: status.code().unwrap_or(-1),
        });
    }

    Ok(())
}
