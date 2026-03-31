use anyhow::{Result, anyhow};

use std::io::{self, IsTerminal, Write};

pub(super) fn wait_to_open_browser() {
    println!();
    print!("Press ENTER to open browser... ");
    let _ = io::stdout().flush();

    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
}

pub(super) fn read_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let value = input.trim().to_string();
    if value.is_empty() {
        return Err(anyhow!("{} is required", prompt.trim_end_matches(':')));
    }

    Ok(value)
}

pub(super) fn read_password(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;

    let password = if io::stdin().is_terminal() {
        rpassword::read_password()?
    } else {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().to_string()
    };

    if password.is_empty() {
        return Err(anyhow!("Password is required"));
    }

    Ok(password)
}

pub(super) fn prompt_otp() -> Result<String> {
    println!();
    let otp = read_line("One-time password: ")?;
    Ok(otp.replace(' ', ""))
}
