use super::GscServer;

impl GscServer {
    pub(crate) async fn handle_api_reference(&self) -> String {
        let reference = serde_json::json!({
            "dimensions": [
                {"name": "query", "description": "Search terms users typed. Most common dimension for keyword analysis."},
                {"name": "page", "description": "Landing page URLs. Use to analyze page-level performance."},
                {"name": "country", "description": "Three-letter country codes (USA, GBR, etc). Use for geo analysis."},
                {"name": "device", "description": "DESKTOP, MOBILE, or TABLET. Use for device-specific performance."},
                {"name": "searchAppearance", "description": "Rich result types (FAQ, VIDEO, etc). Use to measure structured data impact."},
                {"name": "date", "description": "Daily breakdown. Use with other dimensions for trend analysis."},
                {"name": "hour", "description": "Hourly breakdown (0-23, PT timezone). Only available for last 3 days with data_state='hourly_all'."}
            ],
            "metrics": [
                {"name": "clicks", "description": "Number of clicks from search results to your site.", "type": "integer", "range": "0+"},
                {"name": "impressions", "description": "Number of times your site appeared in search results.", "type": "integer", "range": "0+"},
                {"name": "ctr", "description": "Click-through rate (clicks / impressions).", "type": "float", "range": "0.0 - 1.0"},
                {"name": "position", "description": "Average ranking position in search results.", "type": "float", "range": "1.0+ (lower is better)"}
            ]
        });
        serde_json::to_string_pretty(&reference).unwrap_or_else(|e| format!("Error: {e}"))
    }
}
