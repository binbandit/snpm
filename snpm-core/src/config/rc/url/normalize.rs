pub fn normalize_registry_url(value: &str) -> String {
    let mut url = if value.starts_with("//") {
        format!("https:{}", value)
    } else {
        value.to_string()
    };

    if url.ends_with('/') {
        url.pop();
    }

    let (scheme, rest) = if let Some(rest) = url.strip_prefix("https://") {
        ("https", rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        ("http", rest)
    } else if let Some(rest) = url.strip_prefix("//") {
        ("https", rest)
    } else {
        return url;
    };

    let mut parts = rest.splitn(2, '/');
    let hostport = parts.next().unwrap_or("").to_ascii_lowercase();
    let suffix = parts.next().unwrap_or("");
    let host = strip_default_port(&hostport, scheme);

    if suffix.is_empty() {
        format!("{scheme}://{host}")
    } else {
        format!("{scheme}://{host}/{suffix}")
    }
}

pub(super) fn strip_default_port(hostport: &str, scheme: &str) -> String {
    let Some((host, port)) = hostport.split_once(':') else {
        return hostport.to_string();
    };

    let default_https = scheme == "https" && port == "443";
    let default_http = scheme == "http" && port == "80";
    if default_https || default_http {
        host.to_string()
    } else {
        hostport.to_string()
    }
}
