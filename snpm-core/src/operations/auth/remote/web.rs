use super::super::types::{AuthResult, OpenerFn};
use crate::{Result, SnpmError, http};

use reqwest::StatusCode;
use serde::Deserialize;
use std::time::Duration;

pub(super) async fn web_login(registry: &str, opener: OpenerFn) -> Result<AuthResult> {
    let client = http::create_client()?;
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

    if let Err(error) = opener(&init.login_url) {
        crate::console::warn(&format!("could not open browser: {error}"));
        crate::console::info(&format!("Please visit: {}", init.login_url));
    }

    poll_for_token(&client, &init.done_url).await
}

async fn poll_for_token(client: &reqwest::Client, done_url: &str) -> Result<AuthResult> {
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
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse().ok())
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
