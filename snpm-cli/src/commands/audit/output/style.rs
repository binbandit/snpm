use snpm_core::operations;
use std::env;

pub(super) fn paint(code: &str, text: &str) -> String {
    if use_color() {
        format!("\x1b[{}m{}\x1b[0m", code, text)
    } else {
        text.to_string()
    }
}

pub(super) fn severity_badge(severity: operations::Severity) -> String {
    let (color, label) = match severity {
        operations::Severity::Critical => ("41;97", " CRITICAL "),
        operations::Severity::High => ("101;30", "   HIGH   "),
        operations::Severity::Moderate => ("43;30", " MODERATE "),
        operations::Severity::Low => ("42;30", "   LOW    "),
        operations::Severity::Info => ("46;30", "   INFO   "),
    };

    if use_color() {
        format!("\x1b[{}m{}\x1b[0m", color, label)
    } else {
        format!("[{}]", severity.as_str().to_uppercase())
    }
}

pub(super) fn terminal_width() -> usize {
    env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(80)
}

pub(super) fn print_wrapped(text: &str, width: usize, indent: usize) {
    let prefix = " ".repeat(indent);
    let usable = width.saturating_sub(indent);

    if usable == 0 || text.len() <= usable {
        println!("{}{}", prefix, paint("2", text));
        return;
    }

    let mut remaining = text;
    while !remaining.is_empty() {
        let end = char_boundary(remaining, usable);
        let (line, rest) = remaining.split_at(end);
        println!("{}{}", prefix, paint("2", line));
        remaining = rest;
    }
}

fn use_color() -> bool {
    use std::io::IsTerminal;

    env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}

fn char_boundary(input: &str, max_bytes: usize) -> usize {
    if max_bytes >= input.len() {
        return input.len();
    }

    let mut end = max_bytes;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    end
}
