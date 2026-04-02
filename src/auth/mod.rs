pub mod oauth;
pub mod service_account;
pub mod token_store;

use std::path::PathBuf;

pub const WEBMASTERS_SCOPE: &str = "https://www.googleapis.com/auth/webmasters";
pub const INDEXING_SCOPE: &str = "https://www.googleapis.com/auth/indexing";

#[derive(Debug)]
pub enum AuthError {
    NotConfigured(String),
    RefreshFailed(String),
    InvalidKey(String),
    NetworkError(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::NotConfigured(msg)
            | AuthError::RefreshFailed(msg)
            | AuthError::InvalidKey(msg)
            | AuthError::NetworkError(msg) => write!(f, "{msg}"),
        }
    }
}

// Fix #9: implement std::error::Error for composability
impl std::error::Error for AuthError {}

/// Enum dispatch for auth providers. Avoids dyn trait + async_trait crate.
pub enum AuthProvider {
    OAuth(oauth::OAuthProvider),
    ServiceAccount(service_account::ServiceAccountProvider),
    /// No auth configured. All API calls return Error-as-UI with setup instructions.
    /// Allows the server to start and serve meta tools without credentials.
    None(String),
}

impl AuthProvider {
    pub async fn get_token(&self) -> Result<String, AuthError> {
        match self {
            AuthProvider::OAuth(p) => p.get_token().await,
            AuthProvider::ServiceAccount(p) => p.get_token().await,
            AuthProvider::None(msg) => Err(AuthError::NotConfigured(msg.clone())),
        }
    }

    pub async fn invalidate_token(&self) {
        match self {
            AuthProvider::OAuth(p) => p.invalidate_token().await,
            AuthProvider::ServiceAccount(p) => p.invalidate_token().await,
            AuthProvider::None(_) => {}
        }
    }
}

pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("GSC_MCP_CONFIG_DIR") {
        PathBuf::from(dir)
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".config").join("gsc-mcp-rs")
    } else {
        PathBuf::from(".config").join("gsc-mcp-rs")
    }
}

const NO_AUTH_MESSAGE: &str = "No authentication configured.\n\n\
    Setup options:\n\
    1. OAuth (personal use):\n\
       - Create a Google Cloud project and enable Search Console API\n\
       - Create OAuth 2.0 credentials (Desktop app type)\n\
       - Save as ~/.config/gsc-mcp-rs/oauth_credentials.json:\n\
         {\"client_id\": \"...\", \"client_secret\": \"...\"}\n\
       - Run: gsc-mcp-rs auth\n\n\
    2. Service Account (automation):\n\
       - Create a service account in Google Cloud\n\
       - Grant it access to your Search Console properties\n\
       - Download the JSON key and either:\n\
         - Place at ~/.config/gsc-mcp-rs/service_account.json, or\n\
         - Set GSC_SERVICE_ACCOUNT_PATH=/path/to/key.json";

pub async fn detect_auth() -> AuthProvider {
    let config = config_dir();

    // 1. Service account from env var
    if let Ok(sa_path) = std::env::var("GSC_SERVICE_ACCOUNT_PATH") {
        eprintln!("[gsc-mcp-rs] Using service account from GSC_SERVICE_ACCOUNT_PATH");
        match service_account::ServiceAccountProvider::new(sa_path.as_ref()) {
            Ok(provider) => return AuthProvider::ServiceAccount(provider),
            Err(e) => {
                eprintln!("[gsc-mcp-rs] Service account error: {e}");
                return AuthProvider::None(e.to_string());
            }
        }
    }

    // 2. Service account from config dir
    let sa_file = config.join("service_account.json");
    if sa_file.exists() {
        eprintln!(
            "[gsc-mcp-rs] Using service account from {}",
            sa_file.display()
        );
        match service_account::ServiceAccountProvider::new(&sa_file) {
            Ok(provider) => return AuthProvider::ServiceAccount(provider),
            Err(e) => {
                eprintln!("[gsc-mcp-rs] Service account error: {e}");
                return AuthProvider::None(e.to_string());
            }
        }
    }

    // 3. OAuth token from config dir
    let oauth_creds = config.join("oauth_credentials.json");
    let oauth_token = config.join("oauth_token.json");
    if oauth_token.exists() || oauth_creds.exists() {
        eprintln!("[gsc-mcp-rs] Using OAuth authentication");
        match oauth::OAuthProvider::new() {
            Ok(provider) => return AuthProvider::OAuth(provider),
            Err(e) => {
                eprintln!("[gsc-mcp-rs] OAuth error: {e}");
                return AuthProvider::None(e.to_string());
            }
        }
    }

    // 4. Nothing configured, return NoAuth so server still starts
    eprintln!("[gsc-mcp-rs] {NO_AUTH_MESSAGE}");
    AuthProvider::None(NO_AUTH_MESSAGE.into())
}
