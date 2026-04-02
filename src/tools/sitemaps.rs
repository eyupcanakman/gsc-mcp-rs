use super::GscServer;
use crate::types::{VALID_SITEMAP_ACTIONS, validate_enum, validate_site_url};
use rmcp::schemars;
use serde::Deserialize;

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct ListSitemapsParams {
    pub site_url: String,
    #[schemars(description = "Optional sitemap index URL to filter by.")]
    pub sitemap_index: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct SitemapActionParams {
    pub site_url: String,
    #[schemars(description = "The full sitemap URL (e.g., 'https://example.com/sitemap.xml').")]
    pub sitemap_url: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct ManageSitemapParams {
    #[schemars(description = "Action: 'submit' or 'delete'.")]
    pub action: String,
    pub site_url: String,
    #[schemars(description = "The full sitemap URL (e.g., 'https://example.com/sitemap.xml').")]
    pub sitemap_url: String,
}

impl GscServer {
    pub(crate) async fn handle_list_sitemaps(&self, p: ListSitemapsParams) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        match self
            .client
            .list_sitemaps(&p.site_url, p.sitemap_index.as_deref())
            .await
        {
            Ok(data) => {
                serde_json::to_string_pretty(&data).unwrap_or_else(|e| format!("Error: {e}"))
            }
            Err(e) => e.to_ui_string(),
        }
    }

    pub(crate) async fn handle_submit_sitemap(&self, p: SitemapActionParams) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        match self
            .client
            .submit_sitemap(&p.site_url, &p.sitemap_url)
            .await
        {
            Ok(_) => format!(
                "Successfully submitted sitemap '{}'. Google will re-check it.",
                p.sitemap_url
            ),
            Err(e) if e.status == 409 => {
                format!(
                    "Sitemap '{}' already submitted (submit_sitemap is idempotent).",
                    p.sitemap_url
                )
            }
            Err(e) => e.to_ui_string(),
        }
    }

    pub(crate) async fn handle_delete_sitemap(&self, p: SitemapActionParams) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        match self
            .client
            .delete_sitemap(&p.site_url, &p.sitemap_url)
            .await
        {
            Ok(()) => format!("Successfully removed sitemap '{}'.", p.sitemap_url),
            Err(e) if e.status == 404 => {
                format!(
                    "Sitemap '{}' was already removed (delete_sitemap is idempotent).",
                    p.sitemap_url
                )
            }
            Err(e) => e.to_ui_string(),
        }
    }

    pub(crate) async fn handle_manage_sitemap(&self, p: ManageSitemapParams) -> String {
        if let Err(e) = validate_enum(&p.action, VALID_SITEMAP_ACTIONS, "action") {
            return format!("Error: {e}");
        }
        let params = SitemapActionParams {
            site_url: p.site_url,
            sitemap_url: p.sitemap_url,
        };
        match p.action.as_str() {
            "submit" => self.handle_submit_sitemap(params).await,
            "delete" => self.handle_delete_sitemap(params).await,
            _ => unreachable!(),
        }
    }
}
