use crate::console;
use crate::{Result, SnpmError};
use base64::Engine;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use std::fs;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct IntegritySpec {
    tokens: Vec<IntegrityToken>,
    saw_supported: bool,
    cacheable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct IntegrityCacheKey {
    algorithm: &'static str,
    digest_hex: String,
}

#[derive(Debug, Clone)]
struct IntegrityToken {
    algorithm: IntegrityAlgorithm,
    expected_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntegrityAlgorithm {
    Sha512,
    Sha256,
    Sha1,
}

#[cfg(test)]
pub(super) fn verify_integrity(url: &str, integrity: Option<&str>, bytes: &[u8]) -> Result<()> {
    let Some(spec) = IntegritySpec::parse(integrity) else {
        return Ok(());
    };

    let mut verifier = spec.verifier();
    verifier.update(bytes);
    verifier.finish(url).map(|_| ())
}

pub(super) fn verify_integrity_file(
    url: &str,
    integrity: Option<&str>,
    tarball_path: &Path,
) -> Result<()> {
    let Some(spec) = IntegritySpec::parse(integrity) else {
        return Ok(());
    };

    let mut verifier = spec.verifier();
    let mut file = fs::File::open(tarball_path).map_err(|source| SnpmError::ReadFile {
        path: tarball_path.to_path_buf(),
        source,
    })?;
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| SnpmError::ReadFile {
                path: tarball_path.to_path_buf(),
                source,
            })?;
        if read == 0 {
            break;
        }

        verifier.update(&buffer[..read]);
    }

    verifier.finish(url).map(|_| ())
}

impl IntegritySpec {
    pub(super) fn parse(integrity: Option<&str>) -> Option<Self> {
        let Some(integrity) = integrity.map(str::trim).filter(|value| !value.is_empty()) else {
            return None;
        };

        let mut spec = Self {
            tokens: Vec::new(),
            saw_supported: false,
            cacheable: true,
        };

        for token in integrity.split_whitespace() {
            let token = token.split('?').next().unwrap_or(token);
            let Some((algorithm, expected_digest)) = token.split_once('-') else {
                continue;
            };

            let Some(algorithm) = IntegrityAlgorithm::parse(algorithm) else {
                continue;
            };

            spec.saw_supported = true;
            if base64::engine::general_purpose::STANDARD
                .decode(expected_digest)
                .is_err()
            {
                spec.cacheable = false;
            }

            spec.tokens.push(IntegrityToken {
                algorithm,
                expected_digest: expected_digest.to_string(),
            });
        }

        Some(spec)
    }

    pub(super) fn verifier(&self) -> IntegrityVerifier {
        IntegrityVerifier {
            spec: self.clone(),
            sha512: self
                .tokens
                .iter()
                .any(|token| token.algorithm == IntegrityAlgorithm::Sha512)
                .then(Sha512::new),
            sha256: self
                .tokens
                .iter()
                .any(|token| token.algorithm == IntegrityAlgorithm::Sha256)
                .then(Sha256::new),
            sha1: self
                .tokens
                .iter()
                .any(|token| token.algorithm == IntegrityAlgorithm::Sha1)
                .then(Sha1::new),
        }
    }

    pub(super) fn cache_keys(&self, url: &str) -> Result<Vec<IntegrityCacheKey>> {
        if !self.cacheable {
            return Ok(Vec::new());
        }

        let mut keys = Vec::new();
        for algorithm in [
            IntegrityAlgorithm::Sha512,
            IntegrityAlgorithm::Sha256,
            IntegrityAlgorithm::Sha1,
        ] {
            for token in self
                .tokens
                .iter()
                .filter(|token| token.algorithm == algorithm)
            {
                let digest = token.decode_expected_digest(url)?;
                let key = IntegrityCacheKey::new(algorithm.name(), &digest);
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }

        Ok(keys)
    }

    #[cfg(test)]
    pub(super) fn preferred_cache_key(&self, url: &str) -> Result<Option<IntegrityCacheKey>> {
        Ok(self.cache_keys(url)?.into_iter().next())
    }
}

impl IntegrityAlgorithm {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "sha512" => Some(Self::Sha512),
            "sha256" => Some(Self::Sha256),
            "sha1" => Some(Self::Sha1),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Sha512 => "sha512",
            Self::Sha256 => "sha256",
            Self::Sha1 => "sha1",
        }
    }
}

impl IntegrityToken {
    fn decode_expected_digest(&self, url: &str) -> Result<Vec<u8>> {
        base64::engine::general_purpose::STANDARD
            .decode(&self.expected_digest)
            .map_err(|error| SnpmError::Tarball {
                url: url.to_string(),
                reason: format!("invalid integrity value: {error}"),
            })
    }
}

impl IntegrityCacheKey {
    fn new(algorithm: &'static str, digest: &[u8]) -> Self {
        Self {
            algorithm,
            digest_hex: hex::encode(digest),
        }
    }

    pub(super) fn algorithm(&self) -> &'static str {
        self.algorithm
    }

    pub(super) fn digest_hex(&self) -> &str {
        &self.digest_hex
    }
}

pub(super) struct IntegrityVerifier {
    spec: IntegritySpec,
    sha512: Option<Sha512>,
    sha256: Option<Sha256>,
    sha1: Option<Sha1>,
}

impl IntegrityVerifier {
    pub(super) fn update(&mut self, bytes: &[u8]) {
        if let Some(hasher) = self.sha512.as_mut() {
            hasher.update(bytes);
        }
        if let Some(hasher) = self.sha256.as_mut() {
            hasher.update(bytes);
        }
        if let Some(hasher) = self.sha1.as_mut() {
            hasher.update(bytes);
        }
    }

    pub(super) fn finish(self, url: &str) -> Result<Option<IntegrityCacheKey>> {
        if !self.spec.saw_supported {
            console::warn(&format!(
                "Skipping integrity verification for {}: unsupported algorithm",
                url
            ));
            return Ok(None);
        }

        let actual_sha512 = self.sha512.map(|hasher| hasher.finalize().to_vec());
        let actual_sha256 = self.sha256.map(|hasher| hasher.finalize().to_vec());
        let actual_sha1 = self.sha1.map(|hasher| hasher.finalize().to_vec());

        for token in &self.spec.tokens {
            let actual = match token.algorithm {
                IntegrityAlgorithm::Sha512 => actual_sha512.as_deref(),
                IntegrityAlgorithm::Sha256 => actual_sha256.as_deref(),
                IntegrityAlgorithm::Sha1 => actual_sha1.as_deref(),
            }
            .ok_or_else(|| SnpmError::Internal {
                reason: format!(
                    "missing hasher for supported integrity algorithm {}",
                    token.algorithm.name()
                ),
            })?;

            let expected = token.decode_expected_digest(url)?;

            if actual == expected.as_slice() {
                return Ok(self
                    .spec
                    .cacheable
                    .then(|| IntegrityCacheKey::new(token.algorithm.name(), &expected)));
            }
        }

        Err(SnpmError::Tarball {
            url: url.to_string(),
            reason: "integrity verification failed".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{IntegritySpec, verify_integrity, verify_integrity_file};
    use base64::Engine;
    use sha1::Sha1;
    use sha2::{Digest, Sha256, Sha512};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn verifies_sha512_integrity() {
        let bytes = b"hello world";
        let digest = base64::engine::general_purpose::STANDARD.encode(Sha512::digest(bytes));
        let integrity = format!("sha512-{digest}");

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
        let integrity = format!("sha256-{digest}");

        assert!(verify_integrity("https://example.com/pkg.tgz", Some(&integrity), bytes).is_ok());
    }

    #[test]
    fn verify_integrity_sha1() {
        let bytes = b"test data";
        let digest = base64::engine::general_purpose::STANDARD.encode(Sha1::digest(bytes));
        let integrity = format!("sha1-{digest}");

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
        let integrity = format!("sha512-{sha512_digest} sha512-wrongdigest");

        assert!(verify_integrity("https://example.com/pkg.tgz", Some(&integrity), bytes).is_ok());
    }

    #[test]
    fn integrity_cache_key_prefers_sha512() {
        let bytes = b"test data";
        let sha512_digest = base64::engine::general_purpose::STANDARD.encode(Sha512::digest(bytes));
        let sha256_digest = base64::engine::general_purpose::STANDARD.encode(Sha256::digest(bytes));
        let integrity = format!("sha256-{sha256_digest} sha512-{sha512_digest}");

        let spec = IntegritySpec::parse(Some(&integrity)).unwrap();
        let cache_key = spec
            .preferred_cache_key("https://example.com/pkg.tgz")
            .unwrap()
            .unwrap();

        assert_eq!(cache_key.algorithm(), "sha512");
        assert_eq!(cache_key.digest_hex(), hex::encode(Sha512::digest(bytes)));
    }

    #[test]
    fn verify_integrity_file_streams_from_disk() {
        let dir = tempdir().unwrap();
        let tarball_path = dir.path().join("pkg.tgz");
        let bytes = b"stream me";
        fs::write(&tarball_path, bytes).unwrap();

        let digest = base64::engine::general_purpose::STANDARD.encode(Sha512::digest(bytes));
        let integrity = format!("sha512-{digest}");

        assert!(
            verify_integrity_file(
                "https://example.com/pkg.tgz",
                Some(&integrity),
                &tarball_path
            )
            .is_ok()
        );
    }
}
