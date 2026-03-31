use std::io::{self, Write};

use super::super::logging::{ensure_started, log_prefixed};
use super::super::style::{cyan, dim, is_tty};

pub fn header(command: &str, version: &str) {
    ensure_started();
    let msg = format!("snpm {} v{}", command, version);
    eprintln!("{}", dim(&msg));
    eprintln!();
    log_prefixed("INFO", &msg);
}

pub fn step(message: &str) {
    if is_tty() {
        eprint!("\r\u{1b}[K{}\n", dim(message));
        let _ = io::stderr().flush();
    } else {
        eprintln!("{}", dim(message));
    }
    log_prefixed("STEP", message);
}

pub fn step_with_count(message: &str, count: usize) {
    if is_tty() {
        eprint!("\r\u{1b}[K{} {}\n", message, cyan(&format!("[{}]", count)));
        let _ = io::stderr().flush();
    } else {
        eprintln!("{} {}", message, cyan(&format!("[{}]", count)));
    }
    log_prefixed("STEP", &format!("{} [{}]", message, count));
}

pub fn clear_steps(count: usize) {
    if is_tty() {
        for _ in 0..count {
            eprint!("\u{1b}[1A\u{1b}[2K");
        }
        eprint!("\r");
        let _ = io::stderr().flush();
    }
}

pub fn progress(emoji: &str, message: &str, current: usize, total: usize) {
    if is_tty() {
        eprint!(
            "\r\u{1b}[K{} {} {}",
            emoji,
            dim(message),
            cyan(&format!("[{}/{}]", current, total))
        );
        let _ = io::stderr().flush();
    }
}

pub fn clear_line() {
    if is_tty() {
        eprint!("\r\u{1b}[K");
        let _ = io::stderr().flush();
    }
}
