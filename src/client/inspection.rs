use super::{ApiError, GscClient};

impl GscClient {
    pub async fn inspect_url(
        &self,
        site_url: &str,
        inspection_url: &str,
        language_code: &str,
    ) -> Result<serde_json::Value, ApiError> {
        let url = Self::inspection_url("/urlInspection/index:inspect");
        let body = serde_json::json!({
            "inspectionUrl": inspection_url,
            "siteUrl": site_url,
            "languageCode": language_code,
        });
        self.post(&url, &body).await
    }
}
