use crate::{Result, SnpmConfig, SnpmError};
use base64::Engine;
use futures::StreamExt;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

pub(super) async fn download_and_verify_tarball(
    config: &SnpmConfig,
    url: &str,
    integrity: Option<&str>,
    client: &reqwest::Client,
) -> Result<Vec<u8>> {
    let mut request = client.get(url);

    if let Some(header_value) = config.authorization_header_for_url(url) {
        request = request.header("authorization", header_value);
    }

    let response = request
        .send()
        .await
        .map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?
        .error_for_status()
        .map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?;

    let content_length = response.content_length().unwrap_or(0) as usize;
    let mut buffer = Vec::with_capacity(content_length);

    let algorithm = integrity
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| {
            value
                .split_whitespace()
                .next()
                .and_then(|token| token.split_once('-'))
                .map(|(algorithm, _)| algorithm.to_string())
        });

    let mut sha512_hasher = if algorithm.as_deref() == Some("sha512") {
        Some(Sha512::new())
    } else {
        None
    };
    let mut sha256_hasher = if algorithm.as_deref() == Some("sha256") {
        Some(Sha256::new())
    } else {
        None
    };
    let mut sha1_hasher = if algorithm.as_deref() == Some("sha1") {
        Some(Sha1::new())
    } else {
        None
    };

    let mut body_stream = response.bytes_stream();

    while let Some(chunk_result) = body_stream.next().await {
        let chunk = chunk_result.map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?;

        if let Some(ref mut hasher) = sha512_hasher {
            hasher.update(&chunk);
        }
        if let Some(ref mut hasher) = sha256_hasher {
            hasher.update(&chunk);
        }
        if let Some(ref mut hasher) = sha1_hasher {
            hasher.update(&chunk);
        }

        buffer.extend_from_slice(&chunk);
    }

    if let Some(integrity_str) = integrity.map(str::trim).filter(|value| !value.is_empty()) {
        let computed_digest = sha512_hasher
            .map(|hasher| ("sha512", hasher.finalize().to_vec()))
            .or_else(|| sha256_hasher.map(|hasher| ("sha256", hasher.finalize().to_vec())))
            .or_else(|| sha1_hasher.map(|hasher| ("sha1", hasher.finalize().to_vec())));

        if let Some((algorithm_name, actual_digest)) = computed_digest {
            let matched = integrity_str.split_whitespace().any(|token| {
                let token = token.split('?').next().unwrap_or(token);
                if let Some((algorithm, expected_b64)) = token.split_once('-')
                    && algorithm == algorithm_name
                    && let Ok(expected) =
                        base64::engine::general_purpose::STANDARD.decode(expected_b64)
                {
                    return actual_digest == expected;
                }
                false
            });

            if !matched {
                return Err(SnpmError::Tarball {
                    url: url.to_string(),
                    reason: "integrity verification failed".to_string(),
                });
            }
        }
    }

    Ok(buffer)
}
