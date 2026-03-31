pub(super) fn should_keep_line(line: &str, host: &str, scope: Option<&str>) -> bool {
    let trimmed = line.trim();

    !trimmed.is_empty() && !matches_auth_line(trimmed, host) && !matches_scope_line(trimmed, scope)
}

fn matches_auth_line(line: &str, host: &str) -> bool {
    if !line.starts_with("//") || !line.contains(":_authToken=") {
        return false;
    }

    let Some(after_slashes) = line.strip_prefix("//") else {
        return false;
    };

    let host_part = after_slashes
        .split(":_authToken")
        .next()
        .unwrap_or("")
        .trim_end_matches('/');

    host_part == host
}

fn matches_scope_line(line: &str, scope: Option<&str>) -> bool {
    let Some(scope) = scope else {
        return false;
    };

    line.starts_with('@')
        && line
            .split(":registry=")
            .next()
            .map(|value| value.trim() == scope)
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_auth_line_supports_both_host_formats() {
        assert!(matches_auth_line(
            "//registry.npmjs.org/:_authToken=abc",
            "registry.npmjs.org"
        ));
        assert!(matches_auth_line(
            "//registry.npmjs.org:_authToken=abc",
            "registry.npmjs.org"
        ));
        assert!(!matches_auth_line(
            "//other/:_authToken=abc",
            "registry.npmjs.org"
        ));
    }

    #[test]
    fn matches_scope_line_matches_exact_scope() {
        assert!(matches_scope_line(
            "@acme:registry=https://registry.npmjs.org",
            Some("@acme")
        ));
        assert!(!matches_scope_line(
            "@other:registry=https://registry.npmjs.org",
            Some("@acme")
        ));
        assert!(!matches_scope_line(
            "@acme:registry=https://registry.npmjs.org",
            None
        ));
    }
}
