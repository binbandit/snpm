mod cli;
mod config;
mod manifest;
mod runtime;
mod selection;
mod version;

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    match runtime::run(args) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("snpm-switch: {}", error);
            ExitCode::FAILURE
        }
    }
}
