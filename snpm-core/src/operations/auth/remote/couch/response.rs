use super::super::super::types::AuthResult;
use crate::{Result, SnpmError};

use reqwest::StatusCode;
use serde::Deserialize;

#[derive(Deserialize)]
struct CouchResponse {
    token: Option<String>,
}

pub(super) async fn handle_couch_response(
    response: reqwest::Response,
    endpoint: &str,
    username: &str,
) -> Result<AuthResult> {
    let status = response.status();

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();

        if status == StatusCode::UNAUTHORIZED && body.contains("one-time pass") {
            return Err(SnpmError::OtpRequired);
        }

        return Err(SnpmError::Auth {
            reason: format!("login failed ({status}): {body}"),
        });
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
