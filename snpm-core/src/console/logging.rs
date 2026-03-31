use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static START_TIME: OnceLock<Instant> = OnceLock::new();
static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();

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

pub fn verbose(message: &str) {
    log_prefixed("DEBUG", message);
}

pub(super) fn ensure_started() {
    START_TIME.get_or_init(Instant::now);
}

pub(super) fn log_prefixed(level: &str, message: &str) {
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

fn log_raw(message: &str) -> io::Result<()> {
    if let Some(mutex) = LOG_FILE.get()
        && let Ok(mut file) = mutex.lock()
    {
        writeln!(file, "{}", message)?;
    }

    Ok(())
}
