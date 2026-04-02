use super::{ApiError, GscClient};

impl GscClient {
    pub async fn list_sitemaps(
        &self,
        site_url: &str,
        sitemap_index: Option<&str>,
    ) -> Result<serde_json::Value, ApiError> {
        let encoded = Self::encode_site_url(site_url);
        let mut url = Self::gsc_url(&format!("/sites/{encoded}/sitemaps"));
        if let Some(idx) = sitemap_index {
            url.push_str("?sitemapIndex=");
            url.push_str(&Self::encode_site_url(idx));
        }
        self.get(&url).await
    }

    pub async fn submit_sitemap(
        &self,
        site_url: &str,
        feedpath: &str,
    ) -> Result<serde_json::Value, ApiError> {
        let site_encoded = Self::encode_site_url(site_url);
        let feed_encoded = Self::encode_site_url(feedpath);
        self.put(&Self::gsc_url(&format!(
            "/sites/{site_encoded}/sitemaps/{feed_encoded}"
        )))
        .await
    }

    pub async fn delete_sitemap(&self, site_url: &str, feedpath: &str) -> Result<(), ApiError> {
        let site_encoded = Self::encode_site_url(site_url);
        let feed_encoded = Self::encode_site_url(feedpath);
        self.delete(&Self::gsc_url(&format!(
            "/sites/{site_encoded}/sitemaps/{feed_encoded}"
        )))
        .await
    }
}
