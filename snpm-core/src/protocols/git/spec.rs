use crate::{Result, SnpmError};

pub(super) struct GitSpec {
    pub repo: String,
    pub committish: Option<String>,
}

pub(super) fn parse_git_spec(raw: &str) -> Result<GitSpec> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(SnpmError::ResolutionFailed {
            name: raw.to_string(),
            range: "latest".to_string(),
            reason: "git spec is empty".to_string(),
        });
    }

    let (repo_part, committish) = split_committish(trimmed);
    let repo = normalize_git_repo(repo_part).map_err(|reason| SnpmError::ResolutionFailed {
        name: raw.to_string(),
        range: "latest".to_string(),
        reason,
    })?;

    Ok(GitSpec { repo, committish })
}

fn split_committish(value: &str) -> (&str, Option<String>) {
    if let Some(index) = value.rfind('#') {
        let committish = value[index + 1..].trim();
        let base = value[..index].trim();
        let committish = if committish.is_empty() {
            None
        } else {
            Some(committish.to_string())
        };
        (base, committish)
    } else {
        (value, None)
    }
}

fn normalize_git_repo(raw: &str) -> std::result::Result<String, String> {
    let mut repo = raw.trim().to_string();
    if repo.is_empty() {
        return Err("git repo URL is empty".to_string());
    }

    if let Some(stripped) = repo.strip_prefix("git+") {
        repo = stripped.to_string();
    }

    if let Some(stripped) = repo.strip_prefix("git:") {
        repo = normalize_git_colon(stripped)?;
    }

    if let Some(hosted) = normalize_hosted_repo(&repo) {
        return Ok(hosted);
    }

    Ok(correct_ssh_url(&repo))
}

fn normalize_git_colon(rest: &str) -> std::result::Result<String, String> {
    let cleaned = rest.trim();
    if cleaned.is_empty() {
        return Err("git: URL is missing a repo".to_string());
    }

    if cleaned.starts_with("//") {
        return Ok(format!("git:{cleaned}"));
    }

    if cleaned.contains("://") {
        return Ok(cleaned.to_string());
    }

    if cleaned.contains('@') {
        return Ok(cleaned.to_string());
    }

    if let Some((host, path)) = cleaned.split_once(':') {
        if host.is_empty() || path.is_empty() {
            return Err(format!("git: URL is invalid: {cleaned}"));
        }
        return Ok(format!("ssh://git@{host}/{path}"));
    }

    Ok(format!("git://{cleaned}"))
}

fn correct_ssh_url(url: &str) -> String {
    let Some(rest) = url.strip_prefix("ssh://") else {
        return url.to_string();
    };

    let mut parts = rest.splitn(2, '/');
    let auth_host = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    let Some(index) = auth_host.rfind(':') else {
        return url.to_string();
    };

    let (left, right) = auth_host.split_at(index);
    let candidate = right.trim_start_matches(':');

    if candidate.is_empty() || candidate.chars().all(|c| c.is_ascii_digit()) {
        return url.to_string();
    }

    let mut corrected = String::from("ssh://");
    corrected.push_str(left);
    corrected.push('/');
    corrected.push_str(candidate);
    if !path.is_empty() {
        corrected.push('/');
        corrected.push_str(path);
    }

    corrected
}

fn normalize_hosted_repo(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let (host, repo) = if let Some(repo) = trimmed.strip_prefix("github:") {
        ("github.com", repo)
    } else if let Some(repo) = trimmed.strip_prefix("gitlab:") {
        ("gitlab.com", repo)
    } else if let Some(repo) = trimmed.strip_prefix("bitbucket:") {
        ("bitbucket.org", repo)
    } else if looks_like_bare_hosted_repo(trimmed) {
        ("github.com", trimmed)
    } else {
        return None;
    };

    let repo = repo.trim().trim_matches('/');
    if repo.is_empty() {
        return None;
    }

    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    Some(format!("https://{host}/{repo}.git"))
}

fn looks_like_bare_hosted_repo(spec: &str) -> bool {
    if spec.is_empty()
        || spec.starts_with('@')
        || spec.starts_with('.')
        || spec.starts_with('/')
        || spec.contains("://")
        || spec.contains('\\')
        || spec.contains(' ')
    {
        return false;
    }

    let mut parts = spec.split('/');
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
    use super::parse_git_spec;

    #[test]
    fn parse_git_spec_extracts_committish() {
        let spec = parse_git_spec("https://github.com/acme/pkg.git#main").unwrap();
        assert_eq!(spec.repo, "https://github.com/acme/pkg.git");
        assert_eq!(spec.committish.as_deref(), Some("main"));
    }

    #[test]
    fn parse_git_spec_normalizes_git_plus_prefix() {
        let spec = parse_git_spec("git+https://github.com/acme/pkg.git").unwrap();
        assert_eq!(spec.repo, "https://github.com/acme/pkg.git");
    }

    #[test]
    fn parse_git_spec_normalizes_git_colon_host_path() {
        let spec = parse_git_spec("git:github.com:acme/pkg.git").unwrap();
        assert_eq!(spec.repo, "ssh://git@github.com/acme/pkg.git");
    }

    #[test]
    fn parse_git_spec_preserves_ssh_ports() {
        let spec = parse_git_spec("ssh://git@example.com:2222/acme/pkg.git").unwrap();
        assert_eq!(spec.repo, "ssh://git@example.com:2222/acme/pkg.git");
    }

    #[test]
    fn parse_git_spec_rewrites_scp_like_ssh_urls() {
        let spec = parse_git_spec("ssh://git@example.com:acme/pkg.git").unwrap();
        assert_eq!(spec.repo, "ssh://git@example.com/acme/pkg.git");
    }

    #[test]
    fn parse_git_spec_normalizes_github_prefix() {
        let spec = parse_git_spec("github:acme/pkg#main").unwrap();
        assert_eq!(spec.repo, "https://github.com/acme/pkg.git");
        assert_eq!(spec.committish.as_deref(), Some("main"));
    }

    #[test]
    fn parse_git_spec_normalizes_bare_hosted_shorthand() {
        let spec = parse_git_spec("acme/pkg#v1.0.0").unwrap();
        assert_eq!(spec.repo, "https://github.com/acme/pkg.git");
        assert_eq!(spec.committish.as_deref(), Some("v1.0.0"));
    }
}
