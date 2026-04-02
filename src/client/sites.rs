use super::{ApiError, GscClient};

impl GscClient {
    pub async fn list_sites(&self) -> Result<serde_json::Value, ApiError> {
        self.get(&Self::gsc_url("/sites")).await
    }

    pub async fn get_site(&self, site_url: &str) -> Result<serde_json::Value, ApiError> {
        let encoded = Self::encode_site_url(site_url);
        self.get(&Self::gsc_url(&format!("/sites/{encoded}"))).await
    }

    pub async fn add_site(&self, site_url: &str) -> Result<serde_json::Value, ApiError> {
        let encoded = Self::encode_site_url(site_url);
        self.put(&Self::gsc_url(&format!("/sites/{encoded}"))).await
    }

    pub async fn delete_site(&self, site_url: &str) -> Result<(), ApiError> {
        let encoded = Self::encode_site_url(site_url);
        self.delete(&Self::gsc_url(&format!("/sites/{encoded}")))
            .await
    }
}
