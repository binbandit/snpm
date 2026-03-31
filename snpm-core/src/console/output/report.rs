use super::super::logging::log_prefixed;
use super::super::style::{cyan, dim, green, red, yellow};

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
