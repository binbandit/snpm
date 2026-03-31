use crate::console;
use crate::{Result, SnpmError};
use base64::Engine;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

pub(super) fn verify_integrity(url: &str, integrity: Option<&str>, bytes: &[u8]) -> Result<()> {
    let Some(integrity) = integrity.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };

    let mut matched_supported = false;
    let mut saw_supported = false;

    for token in integrity.split_whitespace() {
        let token = token.split('?').next().unwrap_or(token);
        let Some((algorithm, expected_digest)) = token.split_once('-') else {
            continue;
        };

        let actual = match algorithm {
            "sha512" => {
                saw_supported = true;
                Sha512::digest(bytes).to_vec()
            }
            "sha256" => {
                saw_supported = true;
                Sha256::digest(bytes).to_vec()
            }
            "sha1" => {
                saw_supported = true;
                Sha1::digest(bytes).to_vec()
            }
            _ => continue,
        };

        let expected = base64::engine::general_purpose::STANDARD
            .decode(expected_digest)
            .map_err(|error| SnpmError::Tarball {
                url: url.to_string(),
                reason: format!("invalid integrity value: {error}"),
            })?;

        if actual == expected {
            matched_supported = true;
            break;
        }
    }

    if !saw_supported {
        console::warn(&format!(
            "Skipping integrity verification for {}: unsupported algorithm",
            url
        ));
        return Ok(());
    }

    if matched_supported {
        Ok(())
    } else {
        Err(SnpmError::Tarball {
            url: url.to_string(),
            reason: "integrity verification failed".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::verify_integrity;
    use base64::Engine;
    use sha1::Sha1;
    use sha2::{Digest, Sha256, Sha512};

    #[test]
    fn verifies_sha512_integrity() {
        let bytes = b"hello world";
        let digest = base64::engine::general_purpose::STANDARD.encode(Sha512::digest(bytes));
        let integrity = format!("sha512-{}", digest);

        assert!(
            verify_integrity(
                "https://registry.example.com/pkg.tgz",
                Some(&integrity),
                bytes
            )
            .is_ok()
        );
        assert!(
            verify_integrity(
                "https://registry.example.com/pkg.tgz",
                Some(&integrity),
                b"tampered"
            )
            .is_err()
        );
    }

    #[test]
    fn verify_integrity_sha256() {
        let bytes = b"test data";
        let digest = base64::engine::general_purpose::STANDARD.encode(Sha256::digest(bytes));
        let integrity = format!("sha256-{}", digest);

        assert!(verify_integrity("https://example.com/pkg.tgz", Some(&integrity), bytes).is_ok());
    }

    #[test]
    fn verify_integrity_sha1() {
        let bytes = b"test data";
        let digest = base64::engine::general_purpose::STANDARD.encode(Sha1::digest(bytes));
        let integrity = format!("sha1-{}", digest);

        assert!(verify_integrity("https://example.com/pkg.tgz", Some(&integrity), bytes).is_ok());
    }

    #[test]
    fn verify_integrity_none_is_ok() {
        assert!(verify_integrity("https://example.com/pkg.tgz", None, b"anything").is_ok());
    }

    #[test]
    fn verify_integrity_empty_string_is_ok() {
        assert!(verify_integrity("https://example.com/pkg.tgz", Some(""), b"anything").is_ok());
    }

    #[test]
    fn verify_integrity_whitespace_only_is_ok() {
        assert!(verify_integrity("https://example.com/pkg.tgz", Some("   "), b"anything").is_ok());
    }

    #[test]
    fn verify_integrity_multiple_algorithms() {
        let bytes = b"test data";
        let sha512_digest = base64::engine::general_purpose::STANDARD.encode(Sha512::digest(bytes));
        let integrity = format!("sha512-{} sha512-wrongdigest", sha512_digest);

        assert!(verify_integrity("https://example.com/pkg.tgz", Some(&integrity), bytes).is_ok());
    }
}
