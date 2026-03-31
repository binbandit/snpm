use anyhow::Result;
use snpm_core::{SnpmError, operations};

use std::sync::{Arc, Mutex};

use super::prompt::{prompt_otp, read_line, read_password, wait_to_open_browser};

pub(super) async fn authenticate(
    registry: &str,
    auth_type: operations::AuthType,
) -> Result<operations::AuthResult> {
    let mut cached_credentials: Option<operations::Credentials> = None;
    let mut otp: Option<String> = None;

    loop {
        let opener = create_opener();
        let credentials = cached_credentials.clone();
        let captured_credentials = Arc::new(Mutex::new(None));
        let captured_for_closure = captured_credentials.clone();

        let prompt = move || -> snpm_core::Result<operations::Credentials> {
            if let Some(credentials) = credentials {
                return Ok(credentials);
            }

            let credentials = operations::Credentials {
                username: Some(read_line("Username: ").map_err(auth_error)?),
                password: Some(read_password("Password: ").map_err(auth_error)?),
            };
            *captured_for_closure.lock().unwrap() = Some(credentials.clone());
            Ok(credentials)
        };

        match operations::login_with_fallback(registry, auth_type, opener, prompt, otp.as_deref())
            .await
        {
            Ok(result) => return Ok(result),
            Err(SnpmError::OtpRequired) => {
                if cached_credentials.is_none() {
                    cached_credentials = recover_credentials(&captured_credentials)?;
                }
                otp = Some(prompt_otp()?);
            }
            Err(error) => return Err(error.into()),
        }
    }
}

pub(super) fn extract_host(url: &str) -> &str {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(url)
}

fn create_opener() -> operations::OpenerFn {
    Box::new(|url: &str| {
        wait_to_open_browser();
        open::that(url).map_err(|error| error.to_string())
    })
}

fn recover_credentials(
    captured_credentials: &Arc<Mutex<Option<operations::Credentials>>>,
) -> Result<Option<operations::Credentials>> {
    if let Some(credentials) = captured_credentials.lock().unwrap().take() {
        return Ok(Some(credentials));
    }

    Ok(Some(operations::Credentials {
        username: Some(read_line("Username: ")?),
        password: Some(read_password("Password: ")?),
    }))
}

fn auth_error(error: anyhow::Error) -> SnpmError {
    SnpmError::Auth {
        reason: error.to_string(),
    }
}
