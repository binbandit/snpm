use crate::{Result, SnpmError};

pub(super) fn validate_scope(scope: Option<&str>) -> Result<Option<String>> {
    let Some(scope) = scope.map(str::trim).filter(|scope| !scope.is_empty()) else {
        return Ok(None);
    };

    if !scope.starts_with('@') {
        return Err(SnpmError::Auth {
            reason: format!("scope must start with @ (got: {scope})"),
        });
    }

    if scope.len() == 1 {
        return Err(SnpmError::Auth {
            reason: "scope cannot be just @".into(),
        });
    }

    Ok(Some(scope.to_string()))
}

#[cfg(test)]
mod tests {
    use super::validate_scope;

    #[test]
    fn validate_scope_accepts_absent_scope() {
        assert_eq!(validate_scope(None).unwrap(), None);
        assert_eq!(validate_scope(Some("   ")).unwrap(), None);
    }

    #[test]
    fn validate_scope_rejects_invalid_scope() {
        assert!(validate_scope(Some("scope")).is_err());
        assert!(validate_scope(Some("@")).is_err());
    }
}
