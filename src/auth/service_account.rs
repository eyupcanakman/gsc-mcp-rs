use crate::auth::token_store::{self, OAuthToken};
use crate::auth::{AuthError, INDEXING_SCOPE, WEBMASTERS_SCOPE};
use serde::Deserialize;
use std::path::Path;
use tokio::sync::Mutex;

#[derive(Deserialize)]
struct ServiceAccountKey {
    client_email: String,
    private_key: String,
    token_uri: String,
}

pub struct ServiceAccountProvider {
    client: reqwest::Client,
    key: ServiceAccountKey,
    token: Mutex<Option<OAuthToken>>,
}

impl ServiceAccountProvider {
    pub fn new(key_path: &Path) -> Result<Self, AuthError> {
        let content = std::fs::read_to_string(key_path).map_err(|e| {
            AuthError::NotConfigured(format!(
                "Cannot read service account key at {}: {e}",
                key_path.display()
            ))
        })?;
        let key: ServiceAccountKey = serde_json::from_str(&content)
            .map_err(|e| AuthError::InvalidKey(format!("Invalid service account JSON: {e}")))?;

        // Validate token_uri to prevent redirect attacks
        if key.token_uri != "https://oauth2.googleapis.com/token" {
            return Err(AuthError::InvalidKey(format!(
                "Untrusted token_uri in service account key: '{}'. \
                 Expected https://oauth2.googleapis.com/token",
                key.token_uri
            )));
        }

        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            key,
            token: Mutex::new(None),
        })
    }

    pub async fn get_token(&self) -> Result<String, AuthError> {
        let mut guard = self.token.lock().await;
        if let Some(ref token) = *guard
            && !token.is_expired()
        {
            return Ok(token.access_token.clone());
        }
        let new_token = self.fetch_token().await?;
        let access = new_token.access_token.clone();
        *guard = Some(new_token);
        Ok(access)
    }

    pub async fn invalidate_token(&self) {
        *self.token.lock().await = None;
    }

    async fn fetch_token(&self) -> Result<OAuthToken, AuthError> {
        let now = token_store::now_secs();
        let scope = format!("{WEBMASTERS_SCOPE} {INDEXING_SCOPE}");

        let jwt = build_signed_jwt(
            now,
            &scope,
            &self.key.client_email,
            &self.key.token_uri,
            &self.key.private_key,
        )?;

        let resp = self
            .client
            .post(&self.key.token_uri)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", &jwt),
            ])
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("Token exchange failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AuthError::RefreshFailed(token_store::sanitize_token_error(
                status, &body,
            )));
        }

        #[derive(Deserialize)]
        struct TokenResp {
            access_token: String,
            expires_in: u64,
            token_type: String,
        }

        let tr: TokenResp = resp
            .json()
            .await
            .map_err(|e| AuthError::NetworkError(format!("Invalid token response: {e}")))?;

        Ok(OAuthToken {
            access_token: tr.access_token,
            refresh_token: None,
            expires_at: now + tr.expires_in,
            token_type: tr.token_type,
        })
    }
}

fn build_signed_jwt(
    now: u64,
    scope: &str,
    client_email: &str,
    token_uri: &str,
    private_key: &str,
) -> Result<String, AuthError> {
    let header = serde_json::json!({"alg": "RS256", "typ": "JWT"});
    let claims = serde_json::json!({
        "iss": client_email,
        "scope": scope,
        "aud": token_uri,
        "iat": now,
        "exp": now + 3600,
    });

    let header_b64 = base64url_encode(
        &serde_json::to_vec(&header)
            .map_err(|e| AuthError::InvalidKey(format!("JWT header serialization failed: {e}")))?,
    );
    let claims_b64 = base64url_encode(
        &serde_json::to_vec(&claims)
            .map_err(|e| AuthError::InvalidKey(format!("JWT claims serialization failed: {e}")))?,
    );
    let signing_input = format!("{header_b64}.{claims_b64}");

    let signature = sign_rs256(signing_input.as_bytes(), private_key)?;
    let sig_b64 = base64url_encode(&signature);

    Ok(format!("{signing_input}.{sig_b64}"))
}

/// Sign data with RS256 using the `ring` crate. In-memory, no temp files.
fn sign_rs256(data: &[u8], pem_key: &str) -> Result<Vec<u8>, AuthError> {
    let der = pem_to_pkcs8_der(pem_key)?;
    let key_pair = ring::signature::RsaKeyPair::from_pkcs8(&der).map_err(|e| {
        AuthError::InvalidKey(format!(
            "Invalid RSA private key in service account JSON: {e}"
        ))
    })?;
    let rng = ring::rand::SystemRandom::new();
    let mut sig = vec![0u8; key_pair.public().modulus_len()];
    key_pair
        .sign(&ring::signature::RSA_PKCS1_SHA256, &rng, data, &mut sig)
        .map_err(|e| AuthError::InvalidKey(format!("RSA signing failed: {e}")))?;
    Ok(sig)
}

/// Convert a PEM-encoded PKCS#8 private key to raw DER bytes.
fn pem_to_pkcs8_der(pem: &str) -> Result<Vec<u8>, AuthError> {
    let mut b64 = String::new();
    for line in pem.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("-----") {
            continue;
        }
        b64.push_str(trimmed);
    }
    base64_decode(&b64)
        .map_err(|e| AuthError::InvalidKey(format!("Invalid base64 in PEM private key: {e}")))
}

/// Standard base64 decode (with padding, standard alphabet).
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    const DECODE: [u8; 128] = {
        let mut table = [255u8; 128];
        let mut i = 0u8;
        while i < 26 {
            table[(b'A' + i) as usize] = i;
            table[(b'a' + i) as usize] = i + 26;
            i += 1;
        }
        let mut d = 0u8;
        while d < 10 {
            table[(b'0' + d) as usize] = d + 52;
            d += 1;
        }
        table[b'+' as usize] = 62;
        table[b'/' as usize] = 63;
        table
    };

    let bytes = input.as_bytes();
    let mut result = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;

    for &b in bytes {
        if b == b'=' || b == b'\n' || b == b'\r' {
            continue;
        }
        if b >= 128 || DECODE[b as usize] == 255 {
            return Err(format!("invalid base64 character: {}", b as char));
        }
        buf = (buf << 6) | u32::from(DECODE[b as usize]);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(result)
}

/// Base64url encode (no padding, URL-safe alphabet)
fn base64url_encode(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARSET[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARSET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARSET[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(CHARSET[(triple & 0x3F) as usize] as char);
        }
    }
    result
}
