use crate::registry::RegistryProtocol;
use urlencoding::decode;

pub fn split_protocol_spec(spec: &str) -> Option<(RegistryProtocol, String, String)> {
    if looks_like_hosted_git_url(spec) {
        return Some((
            RegistryProtocol::git(),
            spec.to_string(),
            "latest".to_string(),
        ));
    }

    if let Some(patch) = split_patch_protocol_spec(spec) {
        return Some(patch);
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
        "file" | "link" | "portal" => RegistryProtocol::file(),
        other => RegistryProtocol::custom(other),
    };

    if protocol == RegistryProtocol::file() {
        return Some((protocol, rest.to_string(), "latest".to_string()));
    }

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

fn split_patch_protocol_spec(spec: &str) -> Option<(RegistryProtocol, String, String)> {
    let target = spec.strip_prefix("patch:")?.split('#').next()?;
    if target.is_empty() {
        return None;
    }

    let decoded = decode(target)
        .map(|value| value.into_owned())
        .unwrap_or_else(|_| target.to_string());

    if let Some((name, inner)) = split_patch_locator(&decoded) {
        return split_patch_locator_spec(name, inner);
    }

    split_protocol_spec(&decoded)
}

fn split_patch_locator(target: &str) -> Option<(&str, &str)> {
    let at = if target.starts_with('@') {
        let slash = target.find('/')?;
        let remainder = &target[slash + 1..];
        slash + 1 + remainder.find('@')?
    } else {
        target.find('@')?
    };

    if at == 0 || at + 1 >= target.len() {
        return None;
    }

    Some((&target[..at], &target[at + 1..]))
}

fn split_patch_locator_spec(name: &str, inner: &str) -> Option<(RegistryProtocol, String, String)> {
    if let Some(rest) = inner.strip_prefix("npm:") {
        return Some(split_patch_registry_target(
            name,
            rest,
            RegistryProtocol::npm(),
        ));
    }

    if let Some(rest) = inner.strip_prefix("jsr:") {
        return Some(split_patch_registry_target(
            name,
            rest,
            RegistryProtocol::jsr(),
        ));
    }

    if let Some(result) = split_protocol_spec(inner) {
        return Some(result);
    }

    Some((RegistryProtocol::npm(), name.to_string(), inner.to_string()))
}

fn split_patch_registry_target(
    name: &str,
    rest: &str,
    protocol: RegistryProtocol,
) -> (RegistryProtocol, String, String) {
    if rest.is_empty() {
        return (protocol, name.to_string(), "latest".to_string());
    }

    if looks_like_registry_range(rest) {
        return (protocol, name.to_string(), rest.to_string());
    }

    let protocol_name = protocol.name.clone();
    let inner_spec = format!("{protocol_name}:{rest}");

    split_protocol_spec(&inner_spec).unwrap_or((protocol, name.to_string(), rest.to_string()))
}

fn looks_like_registry_range(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('@')
        && !value.contains('/')
        && !value.contains('@')
        && !value.contains(':')
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

    #[test]
    fn split_portal_protocol_spec_as_file() {
        assert_eq!(
            split_protocol_spec("portal:packages/make-fetch-smaller"),
            Some((
                RegistryProtocol::file(),
                "packages/make-fetch-smaller".to_string(),
                "latest".to_string()
            ))
        );
    }

    #[test]
    fn split_patch_protocol_spec_encoded_npm_target() {
        assert_eq!(
            split_protocol_spec(
                "patch:docusaurus-plugin-typedoc-api@npm%3A4.4.0#~/.yarn/patches/typedoc.patch"
            ),
            Some((
                RegistryProtocol::npm(),
                "docusaurus-plugin-typedoc-api".to_string(),
                "4.4.0".to_string()
            ))
        );
    }

    #[test]
    fn split_patch_protocol_spec_local_version_target() {
        assert_eq!(
            split_protocol_spec("patch:yoga-layout-prebuilt@1.10.0#./.yarn/patches/yoga.patch"),
            Some((
                RegistryProtocol::npm(),
                "yoga-layout-prebuilt".to_string(),
                "1.10.0".to_string()
            ))
        );
    }
}
