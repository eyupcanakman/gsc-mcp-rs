use super::GscServer;
use crate::auth::oauth;
use crate::types::{VALID_SITE_ACTIONS, validate_enum, validate_site_url};
use rmcp::schemars;
use serde::Deserialize;

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct SiteUrlParams {
    #[schemars(description = "Site URL (e.g., 'https://example.com/' or 'sc-domain:example.com')")]
    pub site_url: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct ManageSiteParams {
    #[schemars(description = "Action: 'add' or 'delete'.")]
    pub action: String,
    #[schemars(description = "Site URL (e.g., 'https://example.com/' or 'sc-domain:example.com')")]
    pub site_url: String,
}

impl GscServer {
    pub(crate) async fn handle_list_sites(&self) -> String {
        match self.client.list_sites().await {
            Ok(data) => {
                serde_json::to_string_pretty(&data).unwrap_or_else(|e| format!("Error: {e}"))
            }
            Err(e) => e.to_ui_string(),
        }
    }

    pub(crate) async fn handle_get_site_details(&self, params: SiteUrlParams) -> String {
        if let Err(e) = validate_site_url(&params.site_url) {
            return format!("Error: {e}");
        }
        match self.client.get_site(&params.site_url).await {
            Ok(data) => {
                serde_json::to_string_pretty(&data).unwrap_or_else(|e| format!("Error: {e}"))
            }
            Err(e) => e.to_ui_string(),
        }
    }

    pub(crate) async fn handle_add_site(&self, params: SiteUrlParams) -> String {
        if let Err(e) = validate_site_url(&params.site_url) {
            return format!("Error: {e}");
        }
        match self.client.add_site(&params.site_url).await {
            Ok(_) => format!("Successfully added property '{}'.", params.site_url),
            Err(e) if e.status == 409 => {
                format!(
                    "Property '{}' already exists (add_site is idempotent).",
                    params.site_url
                )
            }
            Err(e) => e.to_ui_string(),
        }
    }

    pub(crate) async fn handle_delete_site(&self, params: SiteUrlParams) -> String {
        if let Err(e) = validate_site_url(&params.site_url) {
            return format!("Error: {e}");
        }
        match self.client.delete_site(&params.site_url).await {
            Ok(()) => format!("Successfully removed property '{}'.", params.site_url),
            Err(e) if e.status == 404 => {
                format!(
                    "Property '{}' was already removed (delete_site is idempotent).",
                    params.site_url
                )
            }
            Err(e) => e.to_ui_string(),
        }
    }

    pub(crate) async fn handle_reauthenticate(&self) -> String {
        // Invalidate in-memory cached token first
        self.client.auth.invalidate_token().await;

        let config = crate::auth::config_dir();
        let token_path = config.join("oauth_token.json");

        // Check if this is a service account setup
        let is_service_account = std::env::var("GSC_SERVICE_ACCOUNT_PATH").is_ok()
            || config.join("service_account.json").exists();

        if is_service_account {
            return "In-memory token cleared. Service account tokens are automatically refreshed. \
                    If you need to switch accounts, update the service_account.json file and restart the server."
                .into();
        }

        // Clear OAuth token file
        if token_path.exists() {
            let _ = std::fs::remove_file(&token_path);
        }

        if oauth::is_interactive() {
            "OAuth token cleared (both in-memory and on disk). \
             Run 'gsc-mcp-rs auth' in your terminal to re-authenticate with a new account."
                .into()
        } else {
            "OAuth token cleared (both in-memory and on disk). \
             Since this is a non-interactive session, \
             run 'gsc-mcp-rs auth' in a terminal to complete re-authentication."
                .into()
        }
    }

    pub(crate) async fn handle_manage_site(&self, p: ManageSiteParams) -> String {
        if let Err(e) = validate_enum(&p.action, VALID_SITE_ACTIONS, "action") {
            return format!("Error: {e}");
        }
        let params = SiteUrlParams {
            site_url: p.site_url,
        };
        match p.action.as_str() {
            "add" => self.handle_add_site(params).await,
            "delete" => self.handle_delete_site(params).await,
            _ => unreachable!(),
        }
    }
}
