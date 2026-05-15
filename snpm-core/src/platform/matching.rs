use super::{current_cpu, current_libc, current_os};

pub fn matches_os(list: &[String]) -> bool {
    is_compatible(list, &[])
}

pub fn matches_cpu(list: &[String]) -> bool {
    is_compatible(&[], list)
}

pub fn is_compatible(os: &[String], cpu: &[String]) -> bool {
    is_compatible_with_libc(os, cpu, &[])
}

pub fn is_compatible_with_libc(os: &[String], cpu: &[String], libc: &[String]) -> bool {
    let current_os = current_os();
    let current_cpu = current_cpu();
    let current_libc = current_libc();

    check_platform_list(os, current_os)
        && check_platform_list(cpu, current_cpu)
        && (current_libc == "unknown" || check_platform_list(libc, current_libc))
}

pub(super) fn check_platform_list(list: &[String], current: &str) -> bool {
    if list.len() == 1 && list[0] == "any" {
        return true;
    }

    if list.is_empty() {
        return true;
    }

    let has_positive = list.iter().any(|entry| !entry.starts_with('!'));

    for entry in list {
        if let Some(negated) = entry.strip_prefix('!')
            && negated == current
        {
            return false;
        }
    }

    if !has_positive {
        return true;
    }

    list.iter()
        .any(|entry| !entry.starts_with('!') && entry.as_str() == current)
}
