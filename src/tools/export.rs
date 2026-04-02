use super::GscServer;
use crate::output;
use crate::types::*;
use rmcp::schemars;
use serde::Deserialize;

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct ExportAnalyticsParams {
    pub site_url: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub days: Option<u32>,
    pub dimensions: Option<Vec<String>>,
    pub search_type: Option<String>,
    pub filters: Option<Vec<Filter>>,
    #[schemars(description = "Max rows. Default: 25000.")]
    pub row_limit: Option<u32>,
    #[schemars(description = "Custom filename for the CSV export.")]
    pub filename: Option<String>,
}

impl GscServer {
    pub(crate) async fn handle_export_analytics(&self, p: ExportAnalyticsParams) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        let (start, end) =
            match resolve_dates(p.start_date.as_deref(), p.end_date.as_deref(), p.days, 28) {
                Ok(d) => d,
                Err(e) => return format!("Error: {e}"),
            };
        let dims = p.dimensions.unwrap_or_else(|| vec!["query".into()]);
        if let Err(e) = validate_enum_list(&dims, VALID_DIMENSIONS, "dimension") {
            return format!("Error: {e}");
        }
        let search_type = p.search_type.as_deref().unwrap_or("web");
        if let Err(e) = validate_enum(search_type, VALID_SEARCH_TYPES, "search_type") {
            return format!("Error: {e}");
        }
        let filters = p.filters.unwrap_or_default();
        if let Err(e) = validate_filters(&filters) {
            return format!("Error: {e}");
        }
        let limit = match validate_row_limit(p.row_limit, 25000) {
            Ok(l) => l,
            Err(e) => return format!("Error: {e}"),
        };
        let filename = p.filename.as_deref().unwrap_or("gsc_export");

        let mut body = serde_json::json!({
            "startDate": start, "endDate": end,
            "dimensions": dims, "type": search_type,
            "rowLimit": limit, "dataState": "all",
        });
        if !filters.is_empty() {
            body["dimensionFilterGroups"] = filters_to_groups(&filters);
        }

        match self.client.query_search_analytics(&p.site_url, &body).await {
            Ok(data) => {
                // Force CSV export by setting inline_limit to 0
                output::format_response(&data, "rows", 0, Some(filename), Some(&dims))
            }
            Err(e) => e.to_ui_string(),
        }
    }
}
