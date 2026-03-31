use anyhow::Result;
use std::io::{self, BufRead, Write};

pub(super) fn prompt_confirmation() -> Result<bool> {
    println!();
    print!("Continue? [y/N] ");
    io::stdout().flush()?;

    let stdin = io::stdin();
    let mut input = String::new();
    stdin.lock().read_line(&mut input)?;

    let response = input.trim().to_lowercase();
    Ok(response == "y" || response == "yes")
}
