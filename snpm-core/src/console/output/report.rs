use super::super::logging::log_prefixed;
use super::super::style::{cyan, dim, green, is_tty, red, yellow};
use std::io::{self, Write};

/// Wipe any in-place status line (drawn by `status::progress`, which
/// leaves the cursor mid-line with no trailing newline) before printing
/// a standalone message, so warnings/errors don't get appended to it.
fn clear_pending_status_line() {
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
    let speed_str = summary_speed(count, seconds);

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
    clear_pending_status_line();
    let tag = yellow("warn");
    eprintln!("{} {}", tag, message);
    log_prefixed("WARN", message);
}

pub fn error(message: &str) {
    clear_pending_status_line();
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
    let noun = if count == 1 {
        "dependency"
    } else {
        "dependencies"
    };
    let msg = format!(
        "Blocked install scripts for {} {}. Set SNPM_ALLOW_SCRIPTS to a comma-separated allowlist, for example: {}",
        count,
        noun,
        packages
            .iter()
            .take(1)
            .map(|package| format!("SNPM_ALLOW_SCRIPTS={package}"))
            .next()
            .unwrap_or_else(|| "SNPM_ALLOW_SCRIPTS=package-name".to_string())
    );
    println!("{}", dim(&msg));
    log_prefixed("INFO", &msg);
}

fn summary_speed(count: usize, seconds: f32) -> String {
    if seconds <= 0.0 {
        return String::new();
    }

    let speed = count as f32 / seconds;
    if speed >= 1.0 {
        format!("{:.0} packages/s", speed)
    } else {
        format!("{:.1} packages/s", speed)
    }
}
