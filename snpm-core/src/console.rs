use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static START_TIME: OnceLock<Instant> = OnceLock::new();
static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();

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

pub fn init_logging(path: &Path) -> io::Result<()> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;

    if LOG_FILE.set(Mutex::new(file)).is_err() {
        return Ok(());
    }

    let _ = log_raw("========================================");
    Ok(())
}

pub fn is_logging_enabled() -> bool {
    LOG_FILE.get().is_some()
}

fn log_raw(message: &str) -> io::Result<()> {
    if let Some(mutex) = LOG_FILE.get()
        && let Ok(mut file) = mutex.lock()
    {
        writeln!(file, "{}", message)?;
    }
    Ok(())
}

fn log_prefixed(level: &str, message: &str) {
    if LOG_FILE.get().is_none() {
        return;
    }

    let start = *START_TIME.get_or_init(Instant::now);
    let millis = start.elapsed().as_millis();
    let secs = millis / 1000;
    let ms = millis % 1000;
    let line = format!("[{:>5}.{:03}] [{:<5}] {}", secs, ms, level, message);
    let _ = log_raw(&line);
}

pub fn verbose(message: &str) {
    log_prefixed("DEBUG", message);
}

pub fn header(command: &str, version: &str) {
    START_TIME.get_or_init(Instant::now);
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

pub fn added(name: &str, version: &str, dev: bool) {
    let mark = green("+");
    let dev_label = if dev { dim(" (dev)") } else { String::new() };
    println!(
        "{} {}{}{}",
        mark,
        name,
        dim(&format!("@{}", version)),
        dev_label
    );
    log_prefixed(
        "INFO",
        &format!(
            "added {}@{}{}",
            name,
            version,
            if dev { " (dev)" } else { "" }
        ),
    );
}

pub fn removed(name: &str) {
    let mark = red("-");
    println!("{} {}", mark, name);
    log_prefixed("INFO", &format!("removed {}", name));
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
        cyan(&count.to_string()),
        noun,
        dim(&format!("[{}]", time_str)),
        dim(&format!("({})", speed_str))
    );
    log_prefixed(
        "INFO",
        &format!(
            "summary: count={} time={}s speed={}",
            count, seconds, speed_str
        ),
    );
}

pub fn warn(message: &str) {
    let tag = yellow("warn");
    eprintln!("{} {}", tag, message);
    log_prefixed("WARN", message);
}

pub fn error(message: &str) {
    let tag = red("error");
    eprintln!("{} {}", tag, message);
    log_prefixed("ERROR", message);
}

pub fn info(message: &str) {
    println!("{}", message);
    log_prefixed("INFO", message);
}

pub fn blocked_scripts(packages: &[String]) {
    let count = packages.len();
    let noun = if count == 1 { "script" } else { "scripts" };
    let msg = format!(
        "Blocked {} install {}. Set SNPM_ALLOW_SCRIPTS to enable.",
        count, noun
    );
    println!("{}", dim(&msg));
    log_prefixed("INFO", &msg);
}
