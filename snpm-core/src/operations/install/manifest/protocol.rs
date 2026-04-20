use crate::registry::RegistryProtocol;

pub fn detect_manifest_protocol(spec: &str) -> Option<RegistryProtocol> {
    if spec.starts_with("npm:") {
        Some(RegistryProtocol::npm())
    } else if spec.starts_with("file:") || spec.starts_with("link:") {
        Some(RegistryProtocol::file())
    } else if spec.starts_with("jsr:") {
        Some(RegistryProtocol::jsr())
    } else if is_git_spec(spec) {
        Some(RegistryProtocol::git())
    } else {
        None
    }
}

pub fn is_special_protocol_spec(spec: &str) -> bool {
    spec.starts_with("catalog:")
        || spec.starts_with("workspace:")
        || spec.starts_with("npm:")
        || spec.starts_with("link:")
        || spec.starts_with("jsr:")
        || is_git_spec(spec)
}

fn is_git_spec(spec: &str) -> bool {
    spec.starts_with("git:")
        || spec.starts_with("git+")
        || spec.starts_with("github:")
        || spec.starts_with("gitlab:")
        || spec.starts_with("bitbucket:")
        || looks_like_hosted_git_url(spec)
        || looks_like_hosted_git_shorthand(spec)
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
    use super::*;

    #[test]
    fn detect_manifest_protocol_npm() {
        assert_eq!(
            detect_manifest_protocol("npm:foo@^1.0.0"),
            Some(RegistryProtocol::npm())
        );
    }

    #[test]
    fn detect_manifest_protocol_git() {
        assert_eq!(
            detect_manifest_protocol("git+https://github.com/foo/bar.git"),
            Some(RegistryProtocol::git())
        );
    }

    #[test]
    fn detect_manifest_protocol_github_shorthand() {
        assert_eq!(
            detect_manifest_protocol("webpack/tooling#v1.26.1"),
            Some(RegistryProtocol::git())
        );
    }

    #[test]
    fn detect_manifest_protocol_hosted_git_url() {
        assert_eq!(
            detect_manifest_protocol("https://github.com/uNetworking/uWebSockets.js#v20.49.0"),
            Some(RegistryProtocol::git())
        );
    }

    #[test]
    fn detect_manifest_protocol_jsr() {
        assert_eq!(
            detect_manifest_protocol("jsr:@std/path@^1.0.0"),
            Some(RegistryProtocol::jsr())
        );
    }

    #[test]
    fn detect_manifest_protocol_file() {
        assert_eq!(
            detect_manifest_protocol("file:../local-pkg"),
            Some(RegistryProtocol::file())
        );
    }

    #[test]
    fn detect_manifest_protocol_link() {
        assert_eq!(
            detect_manifest_protocol("link:../local-pkg"),
            Some(RegistryProtocol::file())
        );
    }

    #[test]
    fn detect_manifest_protocol_none() {
        assert_eq!(detect_manifest_protocol("^1.0.0"), None);
    }

    #[test]
    fn is_special_protocol_spec_examples() {
        assert!(is_special_protocol_spec("catalog:"));
        assert!(is_special_protocol_spec("catalog:build"));
        assert!(is_special_protocol_spec("workspace:*"));
        assert!(is_special_protocol_spec("npm:other@^1.0.0"));
        assert!(is_special_protocol_spec("link:../local-pkg"));
        assert!(is_special_protocol_spec("git+https://example.com/repo.git"));
        assert!(is_special_protocol_spec("jsr:@std/path@^1.0.0"));
        assert!(!is_special_protocol_spec("^1.0.0"));
        assert!(!is_special_protocol_spec("~2.0.0"));
    }
}
