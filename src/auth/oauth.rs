use crate::auth::token_store::{self, OAuthToken};
use crate::auth::{AuthError, INDEXING_SCOPE, WEBMASTERS_SCOPE, config_dir};
use crate::types::urlencode;
use serde::Deserialize;
use std::path::PathBuf;
use tokio::sync::Mutex;

#[derive(Deserialize)]
struct OAuthCredentials {
    client_id: String,
    client_secret: String,
}

pub struct OAuthProvider {
    client: reqwest::Client,
    credentials: OAuthCredentials,
    // Fix #6: removed redundant Arc. Provider is already Arc-wrapped in AuthProvider.
    token: Mutex<Option<OAuthToken>>,
    token_path: PathBuf,
}

impl OAuthProvider {
    pub fn new() -> Result<Self, AuthError> {
        let config = config_dir();
        let creds_path = config.join("oauth_credentials.json");
        let token_path = config.join("oauth_token.json");

        let creds_content = std::fs::read_to_string(&creds_path).map_err(|_| {
            AuthError::NotConfigured(format!(
                "OAuth credentials not found at {}.\n\
                 Create this file with: {{\"client_id\": \"...\", \"client_secret\": \"...\"}}\n\
                 Get these from Google Cloud Console > APIs & Services > Credentials > OAuth 2.0 Client ID (Desktop app).",
                creds_path.display()
            ))
        })?;

        let credentials: OAuthCredentials = serde_json::from_str(&creds_content)
            .map_err(|e| AuthError::InvalidKey(format!("Invalid oauth_credentials.json: {e}")))?;

        let token = token_store::read_token(&token_path).ok();

        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            credentials,
            token: Mutex::new(token),
            token_path,
        })
    }

    pub async fn get_token(&self) -> Result<String, AuthError> {
        let mut guard = self.token.lock().await;

        if guard.is_none()
            && let Ok(token) = token_store::read_token(&self.token_path)
        {
            *guard = Some(token);
        }

        match guard.as_ref() {
            None => Err(AuthError::NotConfigured(
                "No OAuth token found. Run 'gsc-mcp-rs auth' to authenticate.".into(),
            )),
            Some(token) if token.is_expired() => {
                let refresh_token = token.refresh_token.as_deref().ok_or_else(|| {
                    AuthError::RefreshFailed(
                        "Token expired and no refresh token available. Run 'gsc-mcp-rs auth' to re-authenticate.".into(),
                    )
                })?;

                eprintln!("[gsc-mcp-rs] Token expired, refreshing...");
                let new_token = token_store::refresh_oauth_token(
                    &self.client,
                    &self.credentials.client_id,
                    &self.credentials.client_secret,
                    refresh_token,
                )
                .await
                .map_err(AuthError::RefreshFailed)?;

                let access = new_token.access_token.clone();
                token_store::write_token(&self.token_path, &new_token)
                    .map_err(AuthError::NetworkError)?;
                *guard = Some(new_token);
                Ok(access)
            }
            Some(token) => Ok(token.access_token.clone()),
        }
    }

    pub async fn invalidate_token(&self) {
        *self.token.lock().await = None;
    }

    /// Interactive OAuth flow. Run from `gsc-mcp-rs auth` command.
    /// MUST only be called when a TTY is available.
    pub async fn run_interactive_flow(&self) -> Result<(), AuthError> {
        let scopes = format!("{WEBMASTERS_SCOPE} {INDEXING_SCOPE}");

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| AuthError::NetworkError(format!("Cannot bind localhost: {e}")))?;
        let port = listener
            .local_addr()
            .map_err(|e| AuthError::NetworkError(format!("Cannot get local address: {e}")))?
            .port();
        // Use 127.0.0.1 instead of localhost. Safari applies HSTS to localhost,
        // upgrading http:// to https:// which breaks the plain-HTTP callback server.
        // Google OAuth allows http://127.0.0.1 for Desktop app redirect URIs.
        let redirect_uri = format!("http://127.0.0.1:{port}");

        let auth_url = format!(
            "https://accounts.google.com/o/oauth2/v2/auth\
             ?client_id={}\
             &redirect_uri={}\
             &response_type=code\
             &scope={}\
             &access_type=offline\
             &prompt=consent",
            urlencode(&self.credentials.client_id),
            urlencode(&redirect_uri),
            urlencode(&scopes),
        );

        eprintln!("\n[gsc-mcp-rs] Opening browser for Google OAuth...");
        eprintln!("[gsc-mcp-rs] If the browser doesn't open, visit this URL:\n");
        eprintln!("{auth_url}\n");

        let _ = open_browser(&auth_url);

        // Fix #11: timeout on listener.accept(), 3 minutes
        eprintln!("[gsc-mcp-rs] Waiting for authorization (timeout: 3 minutes)...");
        let accept_result =
            tokio::time::timeout(std::time::Duration::from_secs(180), listener.accept()).await;

        let (stream, _) = match accept_result {
            Ok(Ok(conn)) => conn,
            Ok(Err(e)) => {
                return Err(AuthError::NetworkError(format!(
                    "Failed to accept callback: {e}"
                )));
            }
            Err(_) => {
                return Err(AuthError::NetworkError(
                    "OAuth callback timed out after 3 minutes. Please try again.".into(),
                ));
            }
        };

        let code = read_auth_code(stream).await?;

        let token = self.exchange_code(&code, &redirect_uri).await?;
        token_store::write_token(&self.token_path, &token).map_err(AuthError::NetworkError)?;

        *self.token.lock().await = Some(token);
        eprintln!("[gsc-mcp-rs] Authentication successful! Token saved.");
        Ok(())
    }

    async fn exchange_code(&self, code: &str, redirect_uri: &str) -> Result<OAuthToken, AuthError> {
        let resp = self
            .client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("code", code),
                ("client_id", self.credentials.client_id.as_str()),
                ("client_secret", self.credentials.client_secret.as_str()),
                ("redirect_uri", redirect_uri),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(format!("Token exchange failed: {e}")))?;

        if !resp.status().is_success() {
            // Fix #4: sanitize error body
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AuthError::RefreshFailed(token_store::sanitize_token_error(
                status, &body,
            )));
        }

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            refresh_token: Option<String>,
            expires_in: u64,
            token_type: String,
        }

        let tr: TokenResponse = resp
            .json()
            .await
            .map_err(|e| AuthError::NetworkError(format!("Invalid token response: {e}")))?;

        Ok(OAuthToken {
            access_token: tr.access_token,
            refresh_token: tr.refresh_token,
            expires_at: token_store::now_secs() + tr.expires_in,
            token_type: tr.token_type,
        })
    }
}

/// Check if stdin is a TTY (interactive terminal).
/// Critical: prevents OAuth hang bug in stdio MCP mode (AminForou #20).
pub fn is_interactive() -> bool {
    #[cfg(unix)]
    {
        // SAFETY: isatty(0) is a well-defined POSIX call, no memory safety concerns
        unsafe { isatty(0) != 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[cfg(unix)]
unsafe extern "C" {
    fn isatty(fd: i32) -> i32;
}

fn open_browser(url: &str) -> Result<(), ()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|_| ())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|_| ())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", url])
            .spawn()
            .map_err(|_| ())?;
    }
    Ok(())
}

async fn read_auth_code(stream: tokio::net::TcpStream) -> Result<String, AuthError> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = stream;
    let mut buf = vec![0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| AuthError::NetworkError(format!("Failed to read callback: {e}")))?;
    let request = String::from_utf8_lossy(&buf[..n]);

    // Extract code from "GET /?code=XXX&... HTTP/1.1"
    let code = extract_query_param(&request, "code")?;

    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
        <html><body><h2>Authentication successful!</h2>\
        <p>You can close this tab and return to the terminal.</p></body></html>";
    let _ = stream.write_all(response.as_bytes()).await;

    Ok(code)
}

fn extract_query_param(request: &str, key: &str) -> Result<String, AuthError> {
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| AuthError::NetworkError("Malformed OAuth callback request.".into()))?;
    let query = path
        .split_once('?')
        .map(|(_, query)| query)
        .ok_or_else(|| {
            AuthError::NetworkError(
                "No authorization code in callback. The user may have denied access.".into(),
            )
        })?;

    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if name == key {
            return percent_decode_query_component(value);
        }
    }

    Err(AuthError::NetworkError(
        "No authorization code in callback. The user may have denied access.".into(),
    ))
}

fn percent_decode_query_component(value: &str) -> Result<String, AuthError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).map_err(|_| {
                    AuthError::NetworkError("Invalid percent-encoding in OAuth callback.".into())
                })?;
                let byte = u8::from_str_radix(hex, 16).map_err(|_| {
                    AuthError::NetworkError("Invalid percent-encoding in OAuth callback.".into())
                })?;
                decoded.push(byte);
                index += 3;
            }
            b'%' => {
                return Err(AuthError::NetworkError(
                    "Invalid percent-encoding in OAuth callback.".into(),
                ));
            }
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8(decoded)
        .map_err(|_| AuthError::NetworkError("OAuth callback contained invalid UTF-8.".into()))
}

#[cfg(test)]
mod tests {
    use super::{OAuthCredentials, OAuthProvider, read_auth_code};
    use crate::auth::token_store::{OAuthToken, write_token};
    use std::path::PathBuf;
    use tokio::sync::Mutex;

    fn temp_path(name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("gsc-mcp-rs-{name}-{unique}.json"))
    }

    #[tokio::test]
    async fn get_token_recovers_from_empty_memory_cache_using_disk_token() {
        let token_path = temp_path("oauth-token");
        write_token(
            &token_path,
            &OAuthToken {
                access_token: "cached-access-token".into(),
                refresh_token: Some("refresh-token".into()),
                expires_at: crate::auth::token_store::now_secs() + 3600,
                token_type: "Bearer".into(),
            },
        )
        .unwrap();

        let provider = OAuthProvider {
            client: reqwest::Client::new(),
            credentials: OAuthCredentials {
                client_id: "id".into(),
                client_secret: "secret".into(),
            },
            token: Mutex::new(None),
            token_path: token_path.clone(),
        };

        let token = provider.get_token().await.unwrap();
        assert_eq!(token, "cached-access-token");

        let _ = std::fs::remove_file(token_path);
    }

    #[tokio::test]
    async fn oauth_callback_decodes_percent_encoded_code() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let client = tokio::spawn(async move {
            let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
            use tokio::io::AsyncWriteExt;
            stream
                .write_all(
                    b"GET /?code=4%2F0AQSTgQ%2Bencoded&scope=test HTTP/1.1\r\nHost: localhost\r\n\r\n",
                )
                .await
                .unwrap();
        });

        let (server_stream, _) = listener.accept().await.unwrap();
        let code = read_auth_code(server_stream).await.unwrap();
        client.await.unwrap();

        assert_eq!(code, "4/0AQSTgQ+encoded");
    }
}
