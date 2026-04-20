use crate::registry::RegistryProtocol;

pub fn split_protocol_spec(spec: &str) -> Option<(RegistryProtocol, String, String)> {
    if looks_like_hosted_git_url(spec) {
        return Some((
            RegistryProtocol::git(),
            spec.to_string(),
            "latest".to_string(),
        ));
    }

    let colon = spec.find(':')?;
    let (prefix, rest) = spec.split_at(colon);
    let rest = &rest[1..];

    if prefix.is_empty() || rest.is_empty() {
        return None;
    }

    if is_git_protocol_prefix(prefix) {
        let source = format!("{prefix}:{rest}");
        return Some((RegistryProtocol::git(), source, "latest".to_string()));
    }

    let protocol = match prefix {
        "npm" => RegistryProtocol::npm(),
        "jsr" => RegistryProtocol::jsr(),
        "file" | "link" => RegistryProtocol::file(),
        other => RegistryProtocol::custom(other),
    };

    let mut source = rest.to_string();
    let mut range = "latest".to_string();

    if let Some(at) = rest.rfind('@') {
        let (name, version_part) = rest.split_at(at);
        if !name.is_empty() {
            source = name.to_string();
        }
        let version = version_part.trim_start_matches('@');
        if !version.is_empty() {
            range = version.to_string();
        }
    }

    Some((protocol, source, range))
}

fn is_git_protocol_prefix(prefix: &str) -> bool {
    matches!(
        prefix,
        "git"
            | "git+http"
            | "git+https"
            | "git+rsync"
            | "git+ftp"
            | "git+file"
            | "git+ssh"
            | "ssh"
            | "github"
            | "gitlab"
            | "bitbucket"
    )
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

#[cfg(test)]
mod tests {
    use super::split_protocol_spec;
    use crate::registry::RegistryProtocol;

    #[test]
    fn split_link_protocol_spec_as_file() {
        assert_eq!(
            split_protocol_spec("link:./scripts/eslint-rules"),
            Some((
                RegistryProtocol::file(),
                "./scripts/eslint-rules".to_string(),
                "latest".to_string()
            ))
        );
    }
}
