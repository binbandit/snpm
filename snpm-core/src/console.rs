use std::env;
use std::sync::OnceLock;

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

fn brand(text: &str) -> String {
    paint("36;1", text)
}

fn bold(text: &str) -> String {
    paint("1", text)
}

fn dim(text: &str) -> String {
    paint("2", text)
}

fn green(text: &str) -> String {
    paint("32", text)
}

fn yellow(text: &str) -> String {
    paint("33", text)
}

fn red(text: &str) -> String {
    paint("31", text)
}

pub fn heading(action: &str) {
    let prefix = brand("snpm");
    let arrow = dim("›");
    let title = bold(action);
    println!("{prefix} {arrow} {title}");
}

pub fn project(name: &str) {
    let label = dim("project");
    let title = bold(name);
    println!("  {label} {title}");
}

pub fn info(message: &str) {
    let mark = dim("›");
    println!("  {mark} {message}");
}

pub fn step(label: &str, detail: &str) {
    let mark = green("✓");
    let text = bold(label);
    if detail.is_empty() {
        println!("  {mark} {text}");
    } else {
        let more = dim(detail);
        println!("  {mark} {text} {more}");
    }
}

pub fn warn(message: &str) {
    let tag = yellow("warn");
    eprintln!("{tag} {message}");
}

pub fn error(message: &str) {
    let tag = red("error");
    eprintln!("{tag} {message}");
}

pub fn added(summary: &str) {
    let mark = green("+");
    println!("  {mark} {summary}");
}

pub fn removed(summary: &str) {
    let mark = red("-");
    println!("  {mark} {summary}");
}

pub fn installed(count: usize, seconds: f32) {
    let mark = green("✓");
    let noun = if count == 1 { "package" } else { "packages" };
    let time = dim(&format!("[{seconds:.2}s]"));
    println!("  {mark} {count} {noun} installed {time}");
}
