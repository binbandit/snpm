mod couch;
mod helpers;
mod web;

use super::types::{AuthResult, AuthType, Credentials, OpenerFn};
use crate::{Result, SnpmError};
use couch::couch_login;
use helpers::require_credential;
use web::web_login;

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
            Err(error) => return Err(error),
        }
    }

    let credentials = prompt_credentials()?;
    let username = require_credential(credentials.username, "username")?;
    let password = require_credential(credentials.password, "password")?;

    couch_login(registry, &username, &password, otp).await
}
