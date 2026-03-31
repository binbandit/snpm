use super::{current_cpu, current_os};

pub fn matches_os(list: &[String]) -> bool {
    is_compatible(list, &[])
}

pub fn matches_cpu(list: &[String]) -> bool {
    is_compatible(&[], list)
}

pub fn is_compatible(os: &[String], cpu: &[String]) -> bool {
    let current_os = current_os();
    let current_cpu = current_cpu();

    check_platform_list(os, current_os) && check_platform_list(cpu, current_cpu)
}

pub(super) fn check_platform_list(list: &[String], current: &str) -> bool {
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
