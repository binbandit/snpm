use std::env;
use std::io::{self, IsTerminal, Write};
use std::sync::OnceLock;
use std::time::Instant;

static START_TIME: OnceLock<Instant> = OnceLock::new();

fn use_color() -> bool {
    static USE_COLOR: OnceLock<bool> = OnceLock::new();
    *USE_COLOR.get_or_init(|| env::var_os("NO_COLOR").is_none())
}

fn is_tty() -> bool {
    static IS_TTY: OnceLock<bool> = OnceLock::new();
    *IS_TTY.get_or_init(|| io::stderr().is_terminal())
}

fn paint(code: &str, text: &str) -> String {
    if use_color() {
        format!("\u{1b}[{}m{}\u{1b}[0m", code, text)
    } else {
        text.to_string()
    }
}

fn dim(text: &str) -> String {
    paint("2", text)
}

fn green(text: &str) -> String {
    paint("32", text)
}

fn cyan(text: &str) -> String {
    paint("36", text)
}

fn yellow(text: &str) -> String {
    paint("33", text)
}

fn red(text: &str) -> String {
    paint("31", text)
}

fn elapsed_ms() -> u128 {
    START_TIME
        .get()
        .map(|t| t.elapsed().as_millis())
        .unwrap_or(0)
}

pub fn header(command: &str) {
    START_TIME.get_or_init(Instant::now);
    eprintln!(
        "{}",
        dim(&format!("snpm {} v{}", command, env!("CARGO_PKG_VERSION")))
    );
    eprintln!();
}

pub fn step(message: &str) {
    if is_tty() {
        eprint!("\r\u{1b}[K{}\n", dim(message));
        let _ = io::stderr().flush();
    } else {
        eprintln!("{}", dim(message));
    }
}

pub fn step_with_count(message: &str, count: usize) {
    if is_tty() {
        eprint!("\r\u{1b}[K{} {}\n", message, cyan(&format!("[{}]", count)));
        let _ = io::stderr().flush();
    } else {
        eprintln!("{} {}", message, cyan(&format!("[{}]", count)));
    }
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

pub fn added(name: &str, version: &str, dev: bool) {
    let mark = green("+");
    let dev_label = if dev { dim(" (dev)") } else { String::new() };
    println!("{} {}@{}{}", mark, name, version, dev_label);
}

pub fn removed(name: &str) {
    let mark = red("-");
    println!("{} {}", mark, name);
}

pub fn summary(count: usize, seconds: f32) {
    println!();
    let time_str = if seconds < 1.0 {
        format!("{:.0}ms", seconds * 1000.0)
    } else {
        format!("{:.2}s", seconds)
    };
    let noun = if count == 1 { "package" } else { "packages" };
    let speed = count as f32 / seconds;
    let speed_str = if speed >= 1.0 {
        format!("{:.0} packages/s", speed)
    } else {
        format!("{:.1} packages/s", speed)
    };
    println!(
        "{} {} installed {} {}",
        count,
        noun,
        dim(&format!("[{}]", time_str)),
        dim(&format!("({})", speed_str))
    );
}

pub fn warn(message: &str) {
    let tag = yellow("warn");
    eprintln!("{} {}", tag, message);
}

pub fn error(message: &str) {
    let tag = red("error");
    eprintln!("{} {}", tag, message);
}

pub fn info(message: &str) {
    println!("{}", message);
}

pub fn blocked_scripts(packages: &[String]) {
    let count = packages.len();
    let noun = if count == 1 { "script" } else { "scripts" };
    println!(
        "{}",
        dim(&format!(
            "Blocked {} install {}. Set SNPM_ALLOW_SCRIPTS to enable.",
            count, noun
        ))
    );
}
