use std::env;

pub fn matches_os(list: &[String]) -> bool {
    is_compatible(list, &[])
}

pub fn matches_cpu(list: &[String]) -> bool {
    is_compatible(&[], list)
}

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

pub fn is_compatible(os: &[String], cpu: &[String]) -> bool {
    let current_os = current_os();
    let current_cpu = current_cpu();

    let os_match = if os.is_empty() {
        true
    } else {
        os.contains(&current_os.to_string())
    };

    let cpu_match = if cpu.is_empty() {
        true
    } else {
        cpu.contains(&current_cpu.to_string())
    };

    os_match && cpu_match
}
