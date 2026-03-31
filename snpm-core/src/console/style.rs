use std::env;
use std::io::{self, IsTerminal};
use std::sync::OnceLock;

pub(super) fn is_tty() -> bool {
    static IS_TTY: OnceLock<bool> = OnceLock::new();
    *IS_TTY.get_or_init(|| io::stderr().is_terminal())
}

fn use_color() -> bool {
    static USE_COLOR: OnceLock<bool> = OnceLock::new();
    *USE_COLOR.get_or_init(|| env::var_os("NO_COLOR").is_none())
}

fn paint(code: &str, text: &str) -> String {
    if use_color() {
        format!("\u{1b}[{}m{}\u{1b}[0m", code, text)
    } else {
        text.to_string()
    }
}

pub(super) fn dim(text: &str) -> String {
    paint("2", text)
}

pub(super) fn green(text: &str) -> String {
    paint("32", text)
}

pub(super) fn cyan(text: &str) -> String {
    paint("36", text)
}

pub(super) fn yellow(text: &str) -> String {
    paint("33", text)
}

pub(super) fn red(text: &str) -> String {
    paint("31", text)
}
