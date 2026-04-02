pub mod indexing;
pub mod inspection;
pub mod search_analytics;
pub mod sitemaps;
pub mod sites;

use crate::auth::AuthProvider;
use std::sync::Arc;

const GSC_BASE: &str = "https://www.googleapis.com/webmasters/v3";
const INSPECTION_BASE: &str = "https://searchconsole.googleapis.com/v1";
const INDEXING_BASE: &str = "https://indexing.googleapis.com/v3";

/// User-facing error from the GSC API, designed for Error-as-UI pattern.
/// Tools convert this to a friendly message string, never to MCP Err().
#[derive(Debug)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl ApiError {
    /// Convert an API error into a user-friendly Error-as-UI string.
    pub fn to_ui_string(&self) -> String {
        match self.status {
            401 => format!(
                "Error: Authentication failed. Your token may be invalid.\n\
                 Try running 'gsc-mcp-rs auth' to re-authenticate.\n\
                 Detail: {}",
                self.message
            ),
            403 => format!(
                "Error: Insufficient permissions.\n\
                 Your account may not have access to this property or operation.\n\
                 Run list_sites to check your permission level.\n\
                 Detail: {}",
                self.message
            ),
            404 => format!(
                "Error: Property or resource not found.\n\
                 Run list_sites to see available properties.\n\
                 If using a domain property, use the format 'sc-domain:example.com'.\n\
                 Detail: {}",
                self.message
            ),
            429 => format!(
                "Error: Google API rate limit hit. Try again in 30 seconds.\n\
                 Reduce row_limit or narrow your date range to avoid hitting limits.\n\
                 Detail: {}",
                self.message
            ),
            _ => format!("Error: {}", self.message),
        }
    }
}

#[derive(Clone)]
pub struct GscClient {
    http: reqwest::Client,
    pub(crate) auth: Arc<AuthProvider>,
}

impl GscClient {
    pub fn new(auth: Arc<AuthProvider>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .connect_timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            auth,
        }
    }

    /// URL-encode a site URL for use in API paths.
    /// Critical for sc-domain: format (sc-domain:example.com -> sc-domain%3Aexample.com)
    pub fn encode_site_url(site_url: &str) -> String {
        crate::types::urlencode(site_url)
    }

    /// Authenticated GET with retry logic.
    pub async fn get(&self, url: &str) -> Result<serde_json::Value, ApiError> {
        self.request(reqwest::Method::GET, url, None).await
    }

    /// Authenticated POST with retry logic.
    pub async fn post(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        self.request(reqwest::Method::POST, url, Some(body)).await
    }

    /// Authenticated PUT with retry logic.
    /// Sends an empty JSON body to satisfy Google's Content-Length requirement.
    pub async fn put(&self, url: &str) -> Result<serde_json::Value, ApiError> {
        self.request(reqwest::Method::PUT, url, Some(&serde_json::json!({})))
            .await
    }

    /// Authenticated DELETE with retry logic.
    pub async fn delete(&self, url: &str) -> Result<(), ApiError> {
        self.request(reqwest::Method::DELETE, url, None).await?;
        Ok(())
    }

    /// Core request method with auth injection and retry logic.
    /// Retry strategy per design doc:
    ///   429: wait Retry-After or exponential backoff from 1s, max 3 retries
    ///   5xx: exponential backoff from 500ms, max 2 retries
    ///   401: refresh token once, retry once
    async fn request(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, ApiError> {
        let mut auth_refreshed = false;

        for attempt in 0..4u32 {
            let token = self.auth.get_token().await.map_err(|e| ApiError {
                status: 0,
                message: e.to_string(),
            })?;

            let mut req = self.http.request(method.clone(), url).bearer_auth(&token);
            if let Some(b) = body {
                req = req.json(b);
            }

            let resp = match req.send().await {
                Ok(r) => r,
                Err(e) => {
                    return Err(ApiError {
                        status: 0,
                        message: format!("Network error: {e}"),
                    });
                }
            };

            let status = resp.status().as_u16();

            match status {
                200..=299 => {
                    let text = resp.text().await.unwrap_or_default();
                    if text.is_empty() {
                        return Ok(serde_json::Value::Null);
                    }
                    return serde_json::from_str(&text).map_err(|e| ApiError {
                        status,
                        message: format!("Invalid JSON response: {e}"),
                    });
                }
                401 if !auth_refreshed => {
                    auth_refreshed = true;
                    self.auth.invalidate_token().await;
                    eprintln!("[gsc-mcp-rs] Token rejected (401), forcing refresh...");
                }
                429 if attempt < 3 => {
                    let retry_after = resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(1u64 << attempt)
                        .min(60); // Cap at 60 seconds to prevent server freezes
                    eprintln!("[gsc-mcp-rs] Rate limited, waiting {retry_after}s...");
                    tokio::time::sleep(std::time::Duration::from_secs(retry_after)).await;
                }
                500..=599 if attempt < 2 => {
                    let wait = 500 * (1u64 << attempt);
                    eprintln!("[gsc-mcp-rs] Server error {status}, retrying in {wait}ms...");
                    tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
                }
                _ => {
                    let body = resp.text().await.unwrap_or_default();
                    // Sanitize: extract error message from Google's JSON error response
                    let detail = serde_json::from_str::<serde_json::Value>(&body)
                        .ok()
                        .and_then(|v| {
                            v.get("error")
                                .and_then(|e| e.get("message"))
                                .and_then(|m| m.as_str())
                                .map(String::from)
                        })
                        .unwrap_or_else(|| format!("HTTP {status}"));
                    return Err(ApiError {
                        status,
                        message: detail,
                    });
                }
            }
        }

        Err(ApiError {
            status: 0,
            message: "Max retries exceeded".into(),
        })
    }

    // --- URL builders for each API base ---

    pub(crate) fn gsc_url(path: &str) -> String {
        format!("{GSC_BASE}{path}")
    }

    pub(crate) fn inspection_url(path: &str) -> String {
        format!("{INSPECTION_BASE}{path}")
    }

    pub(crate) fn indexing_url(path: &str) -> String {
        format!("{INDEXING_BASE}{path}")
    }
}
