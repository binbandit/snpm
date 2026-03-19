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

    let os_match = check_platform_list(os, current_os);
    let cpu_match = check_platform_list(cpu, current_cpu);

    os_match && cpu_match
}

/// Check a platform list against the current value, supporting negation via `!` prefix.
/// An empty list matches everything. If the list contains only negations (e.g. `["!win32"]`),
/// it matches everything except the negated values. If it contains positive entries,
/// the current value must be in the list.
fn check_platform_list(list: &[String], current: &str) -> bool {
    if list.is_empty() {
        return true;
    }

    let has_positive = list.iter().any(|s| !s.starts_with('!'));

    // If any negation matches the current platform, it's excluded
    for entry in list {
        if let Some(negated) = entry.strip_prefix('!') {
            if negated == current {
                return false;
            }
        }
    }

    // If only negations were present and none matched, it's compatible
    if !has_positive {
        return true;
    }

    // Otherwise, the current value must appear in the positive entries
    list.iter()
        .any(|s| !s.starts_with('!') && s.as_str() == current)
}
