use super::{ApiError, GscClient};

impl GscClient {
    pub async fn query_search_analytics(
        &self,
        site_url: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ApiError> {
        let encoded = Self::encode_site_url(site_url);
        let url = Self::gsc_url(&format!("/sites/{encoded}/searchAnalytics/query"));
        self.post(&url, body).await
    }
}
