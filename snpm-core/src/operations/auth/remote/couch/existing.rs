use super::super::super::types::AuthResult;
use super::super::helpers::with_optional_otp;
use crate::{Result, SnpmError, http};

use base64::Engine;

use super::body::couch_user_body;
use super::response::handle_couch_response;

pub(super) async fn couch_login_existing_user(
    registry: &str,
    username: &str,
    password: &str,
    otp: Option<&str>,
) -> Result<AuthResult> {
    let client = http::create_client()?;
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

    let endpoint = format!("{user_url}/-rev/{revision}");
    let auth_header = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"))
    );

    let request = client
        .put(&endpoint)
        .header("Authorization", auth_header)
        .json(&couch_user_body(username, password));
    let response = with_optional_otp(request, otp)
        .send()
        .await
        .map_err(|source| SnpmError::Http {
            url: endpoint.clone(),
            source,
        })?;

    handle_couch_response(response, &endpoint, username).await
}
