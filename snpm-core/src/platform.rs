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
        if let Some(negated) = entry.strip_prefix('!')
            && negated == current
        {
            return false;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_os_returns_known_value() {
        let os = current_os();
        assert!(!os.is_empty());
        // On macOS this should be "darwin"
        #[cfg(target_os = "macos")]
        assert_eq!(os, "darwin");
        #[cfg(target_os = "linux")]
        assert_eq!(os, "linux");
        #[cfg(target_os = "windows")]
        assert_eq!(os, "win32");
    }

    #[test]
    fn current_cpu_returns_known_value() {
        let cpu = current_cpu();
        assert!(!cpu.is_empty());
        #[cfg(target_arch = "x86_64")]
        assert_eq!(cpu, "x64");
        #[cfg(target_arch = "aarch64")]
        assert_eq!(cpu, "arm64");
    }

    #[test]
    fn empty_list_matches_everything() {
        assert!(check_platform_list(&[], "anything"));
    }

    #[test]
    fn positive_match() {
        let list = vec!["darwin".to_string(), "linux".to_string()];
        assert!(check_platform_list(&list, "darwin"));
        assert!(check_platform_list(&list, "linux"));
        assert!(!check_platform_list(&list, "win32"));
    }

    #[test]
    fn negation_excludes() {
        let list = vec!["!win32".to_string()];
        assert!(check_platform_list(&list, "darwin"));
        assert!(check_platform_list(&list, "linux"));
        assert!(!check_platform_list(&list, "win32"));
    }

    #[test]
    fn mixed_positive_and_negation() {
        let list = vec!["darwin".to_string(), "!win32".to_string()];
        assert!(check_platform_list(&list, "darwin"));
        assert!(!check_platform_list(&list, "win32"));
        assert!(!check_platform_list(&list, "linux"));
    }

    #[test]
    fn is_compatible_empty_lists() {
        assert!(is_compatible(&[], &[]));
    }

    #[test]
    fn is_compatible_matching_os() {
        let os = vec![current_os().to_string()];
        assert!(is_compatible(&os, &[]));
    }

    #[test]
    fn is_compatible_non_matching_os() {
        let os = vec!["nonexistent-os".to_string()];
        assert!(!is_compatible(&os, &[]));
    }

    #[test]
    fn is_compatible_matching_cpu() {
        let cpu = vec![current_cpu().to_string()];
        assert!(is_compatible(&[], &cpu));
    }

    #[test]
    fn is_compatible_non_matching_cpu() {
        let cpu = vec!["nonexistent-cpu".to_string()];
        assert!(!is_compatible(&[], &cpu));
    }

    #[test]
    fn is_compatible_both_match() {
        let os = vec![current_os().to_string()];
        let cpu = vec![current_cpu().to_string()];
        assert!(is_compatible(&os, &cpu));
    }

    #[test]
    fn is_compatible_os_match_cpu_no_match() {
        let os = vec![current_os().to_string()];
        let cpu = vec!["nonexistent-cpu".to_string()];
        assert!(!is_compatible(&os, &cpu));
    }

    #[test]
    fn negation_of_current_os_excludes() {
        let os = vec![format!("!{}", current_os())];
        assert!(!is_compatible(&os, &[]));
    }
}
