use super::GscServer;
use crate::output;
use crate::types::*;
use rmcp::schemars;
use serde::Deserialize;

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct SearchAppearanceParams {
    pub site_url: String,
    #[schemars(
        description = "Rich result type. Valid: AMP_BLUE_LINK, AMP_TOP_STORIES, BREADCRUMB, EVENT, FAQ, HOWTO, IMAGE_PACK, JOB_LISTING, MERCHANT_LISTINGS, PRODUCT_SNIPPETS, RECIPE_FEATURE, RECIPE_RICH_SNIPPET, REVIEW_SNIPPET, SITELINKS, VIDEO, WEB_STORY."
    )]
    pub search_appearance: String,
    pub days: Option<u32>,
    pub row_limit: Option<u32>,
}

impl GscServer {
    pub(crate) async fn handle_query_by_search_appearance(
        &self,
        p: SearchAppearanceParams,
    ) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        if let Err(e) = validate_enum(
            &p.search_appearance,
            VALID_SEARCH_APPEARANCES,
            "search_appearance",
        ) {
            return format!("Error: {e}");
        }
        let (start, end) = match resolve_dates(None, None, p.days, 28) {
            Ok(d) => d,
            Err(e) => return format!("Error: {e}"),
        };
        let limit = match validate_row_limit(p.row_limit, 100) {
            Ok(l) => l,
            Err(e) => return format!("Error: {e}"),
        };

        let filters = vec![Filter {
            dimension: "searchAppearance".into(),
            operator: "equals".into(),
            expression: p.search_appearance.clone(),
        }];

        let body = serde_json::json!({
            "startDate": start, "endDate": end,
            "dimensions": ["query", "page"], "type": "web",
            "rowLimit": limit, "dataState": "all",
            "dimensionFilterGroups": filters_to_groups(&filters),
        });

        match self.client.query_search_analytics(&p.site_url, &body).await {
            Ok(data) => {
                let dims = vec!["query".into(), "page".into()];
                output::format_response(
                    &data,
                    "rows",
                    output::DEFAULT_INLINE_LIMIT,
                    None,
                    Some(&dims),
                )
            }
            Err(e) => e.to_ui_string(),
        }
    }
}
