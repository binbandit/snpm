use crate::registry::RegistryProtocol;

pub(in crate::resolve) fn protocol_from_range(range: &str) -> RegistryProtocol {
    if range.starts_with("file:") || range.starts_with("link:") {
        RegistryProtocol::file()
    } else if range.starts_with("jsr:") {
        RegistryProtocol::jsr()
    } else if range.starts_with("git:")
        || range.starts_with("git+")
        || range.starts_with("github:")
        || range.starts_with("gitlab:")
        || range.starts_with("bitbucket:")
        || looks_like_hosted_git_url(range)
        || looks_like_hosted_git_shorthand(range)
    {
        RegistryProtocol::git()
    } else {
        RegistryProtocol::npm()
    }
}

fn looks_like_hosted_git_url(spec: &str) -> bool {
    matches_hosted_git_url(spec, "https://github.com/")
        || matches_hosted_git_url(spec, "https://gitlab.com/")
        || matches_hosted_git_url(spec, "https://bitbucket.org/")
}

fn matches_hosted_git_url(spec: &str, prefix: &str) -> bool {
    let Some(rest) = spec.trim().strip_prefix(prefix) else {
        return false;
    };

    let repo = rest.split('#').next().unwrap_or(rest).trim_matches('/');
    let mut parts = repo.split('/');
    let Some(owner) = parts.next() else {
        return false;
    };
    let Some(name) = parts.next() else {
        return false;
    };

    !owner.is_empty() && !name.is_empty() && parts.next().is_none()
}

fn looks_like_hosted_git_shorthand(spec: &str) -> bool {
    let trimmed = spec.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('@')
        || trimmed.starts_with('.')
        || trimmed.starts_with('/')
        || trimmed.contains("://")
        || trimmed.contains('\\')
        || trimmed.contains(' ')
    {
        return false;
    }

    let repo = trimmed.split('#').next().unwrap_or(trimmed);
    let mut parts = repo.split('/');
    let Some(owner) = parts.next() else {
        return false;
    };
    let Some(name) = parts.next() else {
        return false;
    };

    !owner.is_empty() && !name.is_empty() && parts.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::protocol_from_range;
    use crate::registry::RegistryProtocol;

    #[test]
    fn link_ranges_use_file_protocol() {
        assert_eq!(
            protocol_from_range("link:./scripts/eslint-rules"),
            RegistryProtocol::file()
        );
    }
}
