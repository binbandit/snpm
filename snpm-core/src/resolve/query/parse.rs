use crate::registry::RegistryProtocol;

pub fn split_protocol_spec(spec: &str) -> Option<(RegistryProtocol, String, String)> {
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
        "git" | "git+http" | "git+https" | "git+rsync" | "git+ftp" | "git+file" | "git+ssh" | "ssh"
    )
}
