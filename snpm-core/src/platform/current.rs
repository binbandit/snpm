use std::env;

pub fn current_os() -> &'static str {
    match env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

pub fn current_cpu() -> &'static str {
    match env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    }
}

pub fn current_libc() -> &'static str {
    if env::consts::OS != "linux" {
        return "unknown";
    }

    #[cfg(target_env = "musl")]
    {
        "musl"
    }

    #[cfg(target_env = "gnu")]
    {
        "glibc"
    }

    #[cfg(not(any(target_env = "musl", target_env = "gnu")))]
    {
        "unknown"
    }
}
