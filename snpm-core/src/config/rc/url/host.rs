use super::normalize::strip_default_port;

pub(crate) fn host_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let is_https = trimmed.starts_with("https://");
    let is_http = trimmed.starts_with("http://");
    let hostport = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed)
        .split('/')
        .next()
        .unwrap_or("")
        .trim();

    if hostport.is_empty() {
        return None;
    }

    let host = if is_https {
        strip_default_port(&hostport.to_ascii_lowercase(), "https")
    } else if is_http {
        strip_default_port(&hostport.to_ascii_lowercase(), "http")
    } else {
        hostport.to_ascii_lowercase()
    };

    if host.is_empty() { None } else { Some(host) }
}
