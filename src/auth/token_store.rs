use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: u64,
    pub token_type: String,
}

// Fix #7: manual Debug impl that redacts sensitive fields
impl std::fmt::Debug for OAuthToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthToken")
            .field("access_token", &"[REDACTED]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("expires_at", &self.expires_at)
            .field("token_type", &self.token_type)
            .finish()
    }
}

impl OAuthToken {
    pub fn is_expired(&self) -> bool {
        let now = now_secs();
        now >= self.expires_at.saturating_sub(60) // 60s safety buffer
    }
}

pub fn read_token(path: &Path) -> Result<OAuthToken, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Cannot read token file: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("Invalid token file format: {e}"))
}

// Fix #12: atomic file creation with 0600 from first byte (no TOCTOU window)
pub fn write_token(path: &Path, token: &OAuthToken) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)
                .map_err(|e| format!("Cannot create config directory: {e}"))?;
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create config directory: {e}"))?;
        }
    }

    let content =
        serde_json::to_string_pretty(token).map_err(|e| format!("Cannot serialize token: {e}"))?;

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| format!("Cannot create token file: {e}"))?;
        file.write_all(content.as_bytes())
            .map_err(|e| format!("Cannot write token file: {e}"))?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(path, &content).map_err(|e| format!("Cannot write token file: {e}"))?;
    }

    Ok(())
}

// Fix #4: sanitize error body, extract only the `error` field from Google's response
pub fn sanitize_token_error(status: reqwest::StatusCode, body: &str) -> String {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body)
        && let Some(error) = json.get("error").and_then(|v| v.as_str())
    {
        return format!("Token request failed ({status}): {error}");
    }
    // If body is not JSON or has no error field, show status only
    format!("Token request failed ({status})")
}

pub async fn refresh_oauth_token(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<OAuthToken, String> {
    let resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|e| format!("Token refresh network error: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(sanitize_token_error(status, &body));
    }

    #[derive(Deserialize)]
    struct RefreshResponse {
        access_token: String,
        expires_in: u64,
        token_type: String,
    }

    let r: RefreshResponse = resp
        .json()
        .await
        .map_err(|e| format!("Invalid refresh response: {e}"))?;

    Ok(OAuthToken {
        access_token: r.access_token,
        refresh_token: Some(refresh_token.to_string()),
        expires_at: now_secs() + r.expires_in,
        token_type: r.token_type,
    })
}

// Fix #3: use unwrap_or(0) instead of unwrap() to avoid panic on pre-epoch clock
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
