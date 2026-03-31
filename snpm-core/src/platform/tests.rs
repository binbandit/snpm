use super::matching::check_platform_list;
use super::*;

#[test]
fn current_os_returns_known_value() {
    let os = current_os();
    assert!(!os.is_empty());
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
