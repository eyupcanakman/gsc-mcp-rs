use super::GscServer;
use crate::types::{VALID_NOTIFICATION_TYPES, validate_enum};
use rmcp::schemars;
use serde::Deserialize;

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct RequestIndexingParams {
    #[schemars(description = "The URL to request indexing for.")]
    pub url: String,
    #[schemars(
        description = "Notification type. Default: 'URL_UPDATED'. Valid: URL_UPDATED, URL_DELETED."
    )]
    pub r#type: Option<String>,
}

impl GscServer {
    pub(crate) async fn handle_request_indexing(&self, p: RequestIndexingParams) -> String {
        let ntype = p.r#type.as_deref().unwrap_or("URL_UPDATED");
        if let Err(e) = validate_enum(ntype, VALID_NOTIFICATION_TYPES, "type") {
            return format!("Error: {e}");
        }
        if !p.url.starts_with("http://") && !p.url.starts_with("https://") {
            return "Error: url must start with http:// or https://".into();
        }
        match self.client.publish_url_notification(&p.url, ntype).await {
            Ok(data) => {
                let msg = match ntype {
                    "URL_UPDATED" => format!("Successfully requested indexing for '{}'.", p.url),
                    "URL_DELETED" => format!("Successfully requested removal for '{}'.", p.url),
                    _ => format!("Notification sent for '{}'.", p.url),
                };
                let detail = serde_json::to_string_pretty(&data).unwrap_or_default();
                if detail.len() > 5 {
                    format!("{msg}\n\n{detail}")
                } else {
                    msg
                }
            }
            Err(e) => e.to_ui_string(),
        }
    }
}
