use super::super::env::expand_env_vars;

pub(super) fn parse_rc_entry(line: &str) -> Option<(&str, String)> {
    let Some((key, value)) = line.split_once('=') else {
        // npm's ini parser treats a bare key line (`save-exact`) as the
        // key set to true; dropping such lines silently ignores a valid
        // .npmrc flag.
        let key = line.trim();
        if key.is_empty() || key.starts_with("//") {
            return None;
        }
        return Some((key, "true".to_string()));
    };
    let key = key.trim();
    let mut value = expand_env_vars(value.trim());

    if value.ends_with('/') && !key.starts_with("//") {
        value.pop();
    }

    Some((key, value))
}
