use super::{ApiError, GscClient};

impl GscClient {
    pub async fn publish_url_notification(
        &self,
        url: &str,
        notification_type: &str,
    ) -> Result<serde_json::Value, ApiError> {
        let api_url = Self::indexing_url("/urlNotifications:publish");
        let body = serde_json::json!({
            "url": url,
            "type": notification_type,
        });
        self.post(&api_url, &body).await
    }
}
