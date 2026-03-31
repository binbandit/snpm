use crate::{Result, SnpmError};

pub(super) fn with_optional_otp(
    mut request: reqwest::RequestBuilder,
    otp: Option<&str>,
) -> reqwest::RequestBuilder {
    if let Some(otp_value) = otp {
        request = request.header("npm-otp", otp_value);
    }
    request
}

pub(super) fn require_credential(value: Option<String>, field: &str) -> Result<String> {
    value.ok_or_else(|| SnpmError::Auth {
        reason: format!("{field} is required"),
    })
}

pub(super) fn current_timestamp() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| String::from("1970-01-01T00:00:00Z"))
}
