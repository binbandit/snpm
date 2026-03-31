use super::super::env::expand_env_vars;

pub(super) fn parse_rc_entry(line: &str) -> Option<(&str, String)> {
    let (key, value) = line.split_once('=')?;
    let key = key.trim();
    let mut value = expand_env_vars(value.trim());

    if value.ends_with('/') && !key.starts_with("//") {
        value.pop();
    }

    Some((key, value))
}
