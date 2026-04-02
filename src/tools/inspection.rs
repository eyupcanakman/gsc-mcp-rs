use super::GscServer;
use crate::types::validate_site_url;
use rmcp::schemars;
use serde::Deserialize;

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct InspectUrlParams {
    pub site_url: String,
    #[schemars(description = "The URL to inspect.")]
    pub url: String,
    #[schemars(description = "Language for issue messages. Default: 'en-US'.")]
    pub language_code: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct BatchInspectParams {
    pub site_url: String,
    #[schemars(description = "URLs to inspect (max 50).")]
    pub urls: Vec<String>,
    #[schemars(description = "Concurrent requests (1-20). Default: 5.")]
    pub concurrency: Option<u32>,
    pub language_code: Option<String>,
}

impl GscServer {
    pub(crate) async fn handle_inspect_url(&self, p: InspectUrlParams) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        let lang = p.language_code.as_deref().unwrap_or("en-US");
        match self.client.inspect_url(&p.site_url, &p.url, lang).await {
            Ok(data) => {
                serde_json::to_string_pretty(&data).unwrap_or_else(|e| format!("Error: {e}"))
            }
            Err(e) => e.to_ui_string(),
        }
    }

    pub(crate) async fn handle_batch_inspect_urls(&self, p: BatchInspectParams) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        if p.urls.is_empty() {
            return "Error: urls list cannot be empty.".into();
        }
        if p.urls.len() > 50 {
            return format!(
                "Error: batch_inspect_urls accepts max 50 URLs, got {}.",
                p.urls.len()
            );
        }
        let concurrency = p.concurrency.unwrap_or(5).clamp(1, 20);
        let lang = p.language_code.as_deref().unwrap_or("en-US");

        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency as usize));
        let total = p.urls.len();
        let mut handles = Vec::with_capacity(total);

        for (i, url) in p.urls.into_iter().enumerate() {
            let sem = semaphore.clone();
            let client = self.client.clone();
            let site_url = p.site_url.clone();
            let lang = lang.to_string();
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await;
                eprintln!("[gsc-mcp-rs] Inspecting URL {}/{total}...", i + 1);
                let result = client.inspect_url(&site_url, &url, &lang).await;
                (url, result)
            }));
        }

        let mut results = Vec::new();
        let mut indexed = 0u32;
        let mut not_indexed = 0u32;
        let mut errors = 0u32;

        for handle in handles {
            match handle.await {
                Ok((url, Ok(data))) => {
                    let verdict = data
                        .pointer("/inspectionResult/indexStatusResult/verdict")
                        .and_then(|v| v.as_str())
                        .unwrap_or("UNKNOWN");
                    match verdict {
                        "PASS" => indexed += 1,
                        "FAIL" | "NEUTRAL" => not_indexed += 1,
                        _ => errors += 1,
                    }
                    results.push(format!("  {url} - {verdict}"));
                }
                Ok((url, Err(e))) => {
                    errors += 1;
                    results.push(format!("  {} - Error: {}", url, e.to_ui_string()));
                }
                Err(e) => {
                    errors += 1;
                    results.push(format!("  (task error) - {e}"));
                }
            }
        }

        let mut out = format!("Batch Inspection Results ({total} URLs)\n\n");
        out.push_str(&format!(
            "Summary: {indexed} indexed, {not_indexed} not indexed, {errors} errors\n\n"
        ));
        out.push_str("Details:\n");
        for r in &results {
            out.push_str(r);
            out.push('\n');
        }
        out
    }
}
