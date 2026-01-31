use crate::{Result, SnpmConfig, SnpmError};
use base64::Engine;
use directories::BaseDirs;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthType {
    #[default]
    Web,
    Legacy,
}

#[derive(Debug, Clone)]
pub struct AuthResult {
    pub token: String,
    pub username: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct Credentials {
    pub username: Option<String>,
    pub password: Option<String>,
}

pub type OpenerFn = Box<dyn Fn(&str) -> std::result::Result<(), String> + Send + Sync>;

#[derive(Serialize)]
struct CouchUserBody {
    _id: String,
    name: String,
    password: String,
    #[serde(rename = "type")]
    user_type: &'static str,
    roles: Vec<String>,
    date: String,
}

pub async fn login_with_fallback<F>(
    registry: &str,
    auth_type: AuthType,
    opener: OpenerFn,
    prompt_credentials: F,
    otp: Option<&str>,
) -> Result<AuthResult>
where
    F: FnOnce() -> Result<Credentials>,
{
    if auth_type == AuthType::Web {
        match web_login(registry, opener).await {
            Ok(result) => return Ok(result),
            Err(SnpmError::Auth { reason }) if reason.contains("not supported") => {
                crate::console::verbose("web login not supported, falling back to legacy");
            }
            Err(e) => return Err(e),
        }
    }

    let credentials = prompt_credentials()?;

    let username = credentials.username.ok_or_else(|| SnpmError::Auth {
        reason: "username is required".into(),
    })?;

    let password = credentials.password.ok_or_else(|| SnpmError::Auth {
        reason: "password is required".into(),
    })?;

    couch_login(registry, &username, &password, otp).await
}

async fn web_login(registry: &str, opener: OpenerFn) -> Result<AuthResult> {
    let client = Client::new();
    let endpoint = format!("{}/-/v1/login", registry.trim_end_matches('/'));

    let response = client
        .post(&endpoint)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|source| SnpmError::Http {
            url: endpoint.clone(),
            source,
        })?;

    let status = response.status();
    if status.is_client_error() || status == StatusCode::INTERNAL_SERVER_ERROR {
        return Err(SnpmError::Auth {
            reason: "web login not supported".into(),
        });
    }

    if !status.is_success() {
        return Err(SnpmError::Auth {
            reason: format!("login request failed: {status}"),
        });
    }

    #[derive(Deserialize)]
    struct InitResponse {
        #[serde(rename = "loginUrl")]
        login_url: String,
        #[serde(rename = "doneUrl")]
        done_url: String,
    }

    let init: InitResponse = response.json().await.map_err(|source| SnpmError::Http {
        url: endpoint,
        source,
    })?;

    if !init.login_url.starts_with("http") || !init.done_url.starts_with("http") {
        return Err(SnpmError::Auth {
            reason: "invalid response from login endpoint".into(),
        });
    }

    if let Err(e) = opener(&init.login_url) {
        crate::console::warn(&format!("could not open browser: {e}"));
        crate::console::info(&format!("Please visit: {}", init.login_url));
    }

    poll_for_token(&client, &init.done_url).await
}

async fn poll_for_token(client: &Client, done_url: &str) -> Result<AuthResult> {
    #[derive(Deserialize)]
    struct TokenResponse {
        token: Option<String>,
    }

    for attempt in 1..=120 {
        let response = client
            .get(done_url)
            .send()
            .await
            .map_err(|source| SnpmError::Http {
                url: done_url.to_string(),
                source,
            })?;

        match response.status() {
            StatusCode::OK => {
                let data: TokenResponse =
                    response.json().await.map_err(|source| SnpmError::Http {
                        url: done_url.to_string(),
                        source,
                    })?;

                return data
                    .token
                    .map(|token| AuthResult {
                        token,
                        username: None,
                    })
                    .ok_or_else(|| SnpmError::Auth {
                        reason: "no token in response".into(),
                    });
            }
            StatusCode::ACCEPTED => {
                let wait_seconds = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(2);

                if attempt % 5 == 0 {
                    crate::console::verbose("waiting for authentication...");
                }

                tokio::time::sleep(Duration::from_secs(wait_seconds)).await;
            }
            status => {
                return Err(SnpmError::Auth {
                    reason: format!("authentication failed: {status}"),
                });
            }
        }
    }

    Err(SnpmError::Auth {
        reason: "authentication timed out".into(),
    })
}

async fn couch_login(
    registry: &str,
    username: &str,
    password: &str,
    otp: Option<&str>,
) -> Result<AuthResult> {
    let client = Client::new();
    let endpoint = format!(
        "{}/-/user/org.couchdb.user:{}",
        registry.trim_end_matches('/'),
        urlencoding::encode(username)
    );

    let body = CouchUserBody {
        _id: format!("org.couchdb.user:{username}"),
        name: username.to_string(),
        password: password.to_string(),
        user_type: "user",
        roles: vec![],
        date: current_timestamp(),
    };

    let mut request = client.put(&endpoint).json(&body);
    if let Some(otp_value) = otp {
        request = request.header("npm-otp", otp_value);
    }

    let response = request.send().await.map_err(|source| SnpmError::Http {
        url: endpoint.clone(),
        source,
    })?;

    let status = response.status();

    if status == StatusCode::BAD_REQUEST {
        return Err(SnpmError::Auth {
            reason: format!("no user with username \"{username}\""),
        });
    }

    if status == StatusCode::CONFLICT {
        return couch_login_existing_user(registry, username, password, otp).await;
    }

    handle_couch_response(response, &endpoint, username).await
}

async fn couch_login_existing_user(
    registry: &str,
    username: &str,
    password: &str,
    otp: Option<&str>,
) -> Result<AuthResult> {
    let client = Client::new();
    let user_url = format!(
        "{}/-/user/org.couchdb.user:{}",
        registry.trim_end_matches('/'),
        urlencoding::encode(username)
    );

    let get_response = client
        .get(format!("{user_url}?write=true"))
        .send()
        .await
        .map_err(|source| SnpmError::Http {
            url: user_url.clone(),
            source,
        })?;

    if !get_response.status().is_success() {
        return Err(SnpmError::Auth {
            reason: format!("failed to fetch user: {}", get_response.status()),
        });
    }

    let user_data: serde_json::Value =
        get_response
            .json()
            .await
            .map_err(|source| SnpmError::Http {
                url: user_url.clone(),
                source,
            })?;

    let revision = user_data
        .get("_rev")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| SnpmError::Auth {
            reason: "missing revision in user data".into(),
        })?;

    let body = CouchUserBody {
        _id: format!("org.couchdb.user:{username}"),
        name: username.to_string(),
        password: password.to_string(),
        user_type: "user",
        roles: vec![],
        date: current_timestamp(),
    };

    let auth_header = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"))
    );

    let endpoint = format!("{user_url}/-rev/{revision}");
    let mut request = client
        .put(&endpoint)
        .header("Authorization", auth_header)
        .json(&body);

    if let Some(otp_value) = otp {
        request = request.header("npm-otp", otp_value);
    }

    let response = request.send().await.map_err(|source| SnpmError::Http {
        url: endpoint.clone(),
        source,
    })?;

    handle_couch_response(response, &endpoint, username).await
}

async fn handle_couch_response(
    response: reqwest::Response,
    endpoint: &str,
    username: &str,
) -> Result<AuthResult> {
    let status = response.status();

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();

        if status == StatusCode::UNAUTHORIZED && body.contains("one-time pass") {
            return Err(SnpmError::Auth {
                reason: "OTP required".into(),
            });
        }

        return Err(SnpmError::Auth {
            reason: format!("login failed ({status}): {body}"),
        });
    }

    #[derive(Deserialize)]
    struct CouchResponse {
        token: Option<String>,
    }

    let data: CouchResponse = response.json().await.map_err(|source| SnpmError::Http {
        url: endpoint.to_string(),
        source,
    })?;

    data.token
        .map(|token| AuthResult {
            token,
            username: Some(username.to_string()),
        })
        .ok_or_else(|| SnpmError::Auth {
            reason: "no token in response".into(),
        })
}

fn current_timestamp() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| String::from("1970-01-01T00:00:00Z"))
}

// --- Credential Storage ---

pub fn save_credentials(
    config: &SnpmConfig,
    registry: Option<&str>,
    token: &str,
    scope: Option<&str>,
) -> Result<()> {
    let token = token.trim();
    if token.is_empty() {
        return Err(SnpmError::Auth {
            reason: "token cannot be empty".into(),
        });
    }

    let registry_url = registry
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(config.default_registry.as_str());

    let host = crate::config::host_from_url(registry_url).ok_or_else(|| SnpmError::Auth {
        reason: format!("invalid registry URL: {registry_url}"),
    })?;

    let validated_scope = validate_scope(scope)?;
    let rc_path = get_rc_path();

    let mut lines: Vec<String> = read_rc_file(&rc_path)?
        .into_iter()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && !matches_auth_line(trimmed, &host)
                && !matches_scope_line(trimmed, validated_scope.as_deref())
        })
        .collect();

    lines.push(format!("//{host}/:_authToken={token}"));

    if let Some(ref scope_name) = validated_scope {
        lines.push(format!("{scope_name}:registry={registry_url}"));
    }

    write_rc_file(&rc_path, &lines)
}

pub fn login(
    config: &SnpmConfig,
    registry: Option<&str>,
    token: &str,
    scope: Option<&str>,
) -> Result<()> {
    save_credentials(config, registry, token, scope)
}

pub fn logout(config: &SnpmConfig, registry: Option<&str>, scope: Option<&str>) -> Result<()> {
    let registry_url = registry
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(config.default_registry.as_str());

    let host = crate::config::host_from_url(registry_url).ok_or_else(|| SnpmError::Auth {
        reason: format!("invalid registry URL: {registry_url}"),
    })?;

    let validated_scope = validate_scope(scope)?;
    let rc_path = get_rc_path();

    if !rc_path.is_file() {
        return Ok(());
    }

    let original_lines = read_rc_file(&rc_path)?;
    let filtered_lines: Vec<String> = original_lines
        .iter()
        .filter(|line| {
            let trimmed = line.trim();
            !matches_auth_line(trimmed, &host)
                && !matches_scope_line(trimmed, validated_scope.as_deref())
        })
        .cloned()
        .collect();

    if filtered_lines.len() == original_lines.len() {
        return Err(SnpmError::Auth {
            reason: format!("no credentials found for {registry_url}"),
        });
    }

    write_rc_file(&rc_path, &filtered_lines)
}

fn validate_scope(scope: Option<&str>) -> Result<Option<String>> {
    let Some(s) = scope.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };

    if !s.starts_with('@') {
        return Err(SnpmError::Auth {
            reason: format!("scope must start with @ (got: {s})"),
        });
    }

    if s.len() == 1 {
        return Err(SnpmError::Auth {
            reason: "scope cannot be just @".into(),
        });
    }

    Ok(Some(s.to_string()))
}

fn get_rc_path() -> PathBuf {
    BaseDirs::new()
        .map(|dirs| dirs.home_dir().join(".snpmrc"))
        .unwrap_or_else(|| PathBuf::from(".snpmrc"))
}

fn read_rc_file(path: &PathBuf) -> Result<Vec<String>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }

    fs::read_to_string(path)
        .map(|content| content.lines().map(String::from).collect())
        .map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })
}

fn write_rc_file(path: &PathBuf, lines: &[String]) -> Result<()> {
    let content = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };

    fs::write(path, content).map_err(|source| SnpmError::WriteFile {
        path: path.clone(),
        source,
    })
}

fn matches_auth_line(line: &str, host: &str) -> bool {
    if !line.starts_with("//") || !line.contains(":_authToken=") {
        return false;
    }

    let Some(after_slashes) = line.strip_prefix("//") else {
        return false;
    };

    // Handle both formats: //host/:_authToken and //host:_authToken
    let host_part = after_slashes
        .split(":_authToken")
        .next()
        .unwrap_or("")
        .trim_end_matches('/');

    host_part == host
}

fn matches_scope_line(line: &str, scope: Option<&str>) -> bool {
    let Some(scope) = scope else {
        return false;
    };

    line.starts_with('@')
        && line
            .split(":registry=")
            .next()
            .map(|s| s.trim() == scope)
            .unwrap_or(false)
}
