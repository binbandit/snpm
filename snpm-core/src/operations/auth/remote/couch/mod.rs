mod body;
mod existing;
mod response;

use super::super::types::AuthResult;
use super::helpers::with_optional_otp;
use crate::{Result, SnpmError, http};

use reqwest::StatusCode;

use body::couch_user_body;
use existing::couch_login_existing_user;
use response::handle_couch_response;

pub(super) async fn couch_login(
    registry: &str,
    username: &str,
    password: &str,
    otp: Option<&str>,
) -> Result<AuthResult> {
    let client = http::create_client()?;
    let endpoint = format!(
        "{}/-/user/org.couchdb.user:{}",
        registry.trim_end_matches('/'),
        urlencoding::encode(username)
    );

    let response = with_optional_otp(
        client
            .put(&endpoint)
            .json(&couch_user_body(username, password)),
        otp,
    )
    .send()
    .await
    .map_err(|source| SnpmError::Http {
        url: endpoint.clone(),
        source,
    })?;

    match response.status() {
        StatusCode::BAD_REQUEST => Err(SnpmError::Auth {
            reason: format!("no user with username \"{username}\""),
        }),
        StatusCode::CONFLICT => couch_login_existing_user(registry, username, password, otp).await,
        _ => handle_couch_response(response, &endpoint, username).await,
    }
}
