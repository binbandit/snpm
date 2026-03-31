mod binary;
mod commands;

use crate::cli::{is_meta_command, parse_switch_options};

use std::process::{Command, ExitCode, ExitStatus, Stdio};

use binary::resolve_binary;
use commands::handle_switch_command;

pub(crate) fn run(args: Vec<String>) -> anyhow::Result<ExitCode> {
    let (mut options, args) = parse_switch_options(args)?;

    if args.first().map(|arg| arg.as_str()) == Some("switch") {
        return handle_switch_command(&args[1..], &options);
    }

    if is_meta_command(&args) {
        options.ignore_package_manager = true;
    }

    let snpm_binary = resolve_binary(None, &options)?;

    let mut command = Command::new(&snpm_binary);
    command.args(&args);
    command.stdin(Stdio::inherit());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

    let status = command.status()?;
    Ok(exit_code_from_status(status))
}

fn exit_code_from_status(status: ExitStatus) -> ExitCode {
    status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .map(ExitCode::from)
        .unwrap_or_else(|| ExitCode::from(1))
}
