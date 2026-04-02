use super::GscServer;
use crate::output;
use crate::types::*;
use rmcp::schemars;
use serde::Deserialize;

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct SearchAnalyticsParams {
    #[schemars(description = "Site URL (e.g., 'https://example.com/' or 'sc-domain:example.com')")]
    pub site_url: String,
    #[schemars(description = "Start date (YYYY-MM-DD). Optional if 'days' is set.")]
    pub start_date: Option<String>,
    #[schemars(description = "End date (YYYY-MM-DD). Optional if 'days' is set.")]
    pub end_date: Option<String>,
    #[schemars(
        description = "Number of days to look back (alternative to start_date/end_date). Default: 28."
    )]
    pub days: Option<u32>,
    #[schemars(
        description = "Dimensions to group by. Default: [\"query\"]. Valid: query, page, country, device, searchAppearance, date, hour."
    )]
    pub dimensions: Option<Vec<String>>,
    #[schemars(
        description = "Search type. Default: \"web\". Valid: web, image, video, news, discover, googleNews."
    )]
    pub search_type: Option<String>,
    #[schemars(
        description = "Aggregation type. Valid: auto, byProperty, byPage, byNewsShowcasePanel."
    )]
    pub aggregation_type: Option<String>,
    #[schemars(description = "Max rows to return (1-25000). Default: 1000.")]
    pub row_limit: Option<u32>,
    #[schemars(description = "Pagination offset. Default: 0.")]
    pub start_row: Option<u32>,
    #[schemars(
        description = "Sort by metric. Default: \"clicks\". Valid: clicks, impressions, ctr, position."
    )]
    pub sort_by: Option<String>,
    #[schemars(
        description = "Sort direction. Default: \"descending\". Valid: ascending, descending."
    )]
    pub sort_direction: Option<String>,
    #[schemars(description = "Data freshness. Default: \"all\". Valid: all, final, hourly_all.")]
    pub data_state: Option<String>,
    #[schemars(description = "Filters array: [{dimension, operator, expression}].")]
    pub filters: Option<Vec<Filter>>,
    #[schemars(
        description = "Add an extra dimension for comparison splits (e.g., 'device' for DESKTOP/MOBILE/TABLET, 'country' for geographic). Appended to dimensions if not already present."
    )]
    pub breakdown: Option<String>,
    #[schemars(
        description = "Row threshold for CSV export (default 500). Set higher to keep more data inline."
    )]
    pub inline_limit: Option<u32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct ComparePeriodsParams {
    pub site_url: String,
    #[schemars(description = "Current period start (YYYY-MM-DD)")]
    pub current_start: String,
    #[schemars(description = "Current period end (YYYY-MM-DD)")]
    pub current_end: String,
    #[schemars(description = "Previous period start (YYYY-MM-DD)")]
    pub previous_start: String,
    #[schemars(description = "Previous period end (YYYY-MM-DD)")]
    pub previous_end: String,
    pub dimensions: Option<Vec<String>>,
    #[schemars(description = "Max rows. Default: 50.")]
    pub row_limit: Option<u32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct PerformanceOverviewParams {
    pub site_url: String,
    #[schemars(description = "Days to look back. Default: 28.")]
    pub days: Option<u32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct PageQueryBreakdownParams {
    pub site_url: String,
    #[schemars(description = "The specific page URL to analyze.")]
    pub page_url: String,
    pub days: Option<u32>,
    #[schemars(description = "Max rows. Default: 100.")]
    pub row_limit: Option<u32>,
}

#[expect(clippy::too_many_arguments)]
pub(crate) fn build_analytics_body(
    start_date: &str,
    end_date: &str,
    dimensions: &[String],
    search_type: &str,
    row_limit: u32,
    start_row: u32,
    data_state: &str,
    filters: &[Filter],
    aggregation_type: Option<&str>,
) -> serde_json::Value {
    let mut body = serde_json::json!({
        "startDate": start_date,
        "endDate": end_date,
        "type": search_type,
        "rowLimit": row_limit,
        "startRow": start_row,
        "dataState": data_state,
    });
    // Only include dimensions when non-empty (empty array causes API errors)
    if !dimensions.is_empty() {
        body["dimensions"] = serde_json::json!(dimensions);
    }

    if !filters.is_empty() {
        body["dimensionFilterGroups"] = filters_to_groups(filters);
    }
    if let Some(agg) = aggregation_type {
        body["aggregationType"] = serde_json::Value::String(agg.to_string());
    }

    body
}

pub(crate) const FULL_FETCH_LIMIT: u32 = 25_000;

fn analytics_fetch_plan(
    row_limit: u32,
    start_row: u32,
    sort_by: Option<&str>,
    sort_direction: Option<&str>,
) -> (u32, u32) {
    if sort_by.is_some() || sort_direction.is_some() {
        (FULL_FETCH_LIMIT, 0)
    } else {
        (row_limit, start_row)
    }
}

pub(crate) fn sort_rows_by_metric(rows: &mut [serde_json::Value], sort_by: &str, descending: bool) {
    rows.sort_by(|a, b| {
        let va = metric(a, sort_by);
        let vb = metric(b, sort_by);
        if descending {
            vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal)
        } else {
            va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
}

impl GscServer {
    pub(crate) async fn handle_search_analytics(&self, p: SearchAnalyticsParams) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        let (start, end) =
            match resolve_dates(p.start_date.as_deref(), p.end_date.as_deref(), p.days, 28) {
                Ok(d) => d,
                Err(e) => return format!("Error: {e}"),
            };
        let mut dimensions = p.dimensions.unwrap_or_else(|| vec!["query".into()]);
        if let Err(e) = validate_enum_list(&dimensions, VALID_DIMENSIONS, "dimension") {
            return format!("Error: {e}");
        }
        if let Some(ref bd) = p.breakdown {
            if let Err(e) = validate_enum(bd, VALID_DIMENSIONS, "breakdown") {
                return format!("Error: {e}");
            }
            if !dimensions.contains(bd) {
                dimensions.push(bd.clone());
            }
        }
        let search_type = p.search_type.as_deref().unwrap_or("web");
        if let Err(e) = validate_enum(search_type, VALID_SEARCH_TYPES, "search_type") {
            return format!("Error: {e}");
        }
        if let Some(ref agg) = p.aggregation_type
            && let Err(e) = validate_enum(agg, VALID_AGGREGATION_TYPES, "aggregation_type")
        {
            return format!("Error: {e}");
        }
        let data_state = p.data_state.as_deref().unwrap_or("all");
        if let Err(e) = validate_enum(data_state, VALID_DATA_STATES, "data_state") {
            return format!("Error: {e}");
        }
        let row_limit = match validate_row_limit(p.row_limit, 1000) {
            Ok(l) => l,
            Err(e) => return format!("Error: {e}"),
        };
        let start_row = p.start_row.unwrap_or(0);
        let filters = p.filters.unwrap_or_default();
        if let Err(e) = validate_filters(&filters) {
            return format!("Error: {e}");
        }
        // Validate sort_by if provided
        if let Some(ref sb) = p.sort_by
            && let Err(e) = validate_enum(sb, VALID_METRICS, "sort_by")
        {
            return format!("Error: {e}");
        }
        if let Some(ref sd) = p.sort_direction
            && let Err(e) = validate_enum(sd, &["ascending", "descending"], "sort_direction")
        {
            return format!("Error: {e}");
        }

        let (fetch_limit, fetch_start_row) = analytics_fetch_plan(
            row_limit,
            start_row,
            p.sort_by.as_deref(),
            p.sort_direction.as_deref(),
        );

        let body = build_analytics_body(
            &start,
            &end,
            &dimensions,
            search_type,
            fetch_limit,
            fetch_start_row,
            data_state,
            &filters,
            p.aggregation_type.as_deref(),
        );

        match self.client.query_search_analytics(&p.site_url, &body).await {
            Ok(mut data) => {
                let sort_by = p.sort_by.as_deref().unwrap_or("clicks");
                let default_dir = if sort_by == "position" {
                    "ascending"
                } else {
                    "descending"
                };
                let descending = p.sort_direction.as_deref().unwrap_or(default_dir) == "descending";
                let explicit_sort = p.sort_by.is_some() || p.sort_direction.is_some();

                if let Some(rows) = data.get_mut("rows").and_then(|v| v.as_array_mut())
                    && explicit_sort
                {
                    sort_rows_by_metric(rows, sort_by, descending);
                    let requested_start = start_row as usize;
                    let requested_end = requested_start.saturating_add(row_limit as usize);
                    let len = rows.len();
                    let end = requested_end.min(len);
                    if requested_start < len {
                        let sliced = rows[requested_start..end].to_vec();
                        *rows = sliced;
                    } else {
                        rows.clear();
                    }
                }
                let inline = p.inline_limit.unwrap_or(output::DEFAULT_INLINE_LIMIT);
                output::format_response(&data, "rows", inline, None, Some(&dimensions))
            }
            Err(e) => e.to_ui_string(),
        }
    }

    pub(crate) async fn handle_compare_periods(&self, p: ComparePeriodsParams) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        for d in [
            &p.current_start,
            &p.current_end,
            &p.previous_start,
            &p.previous_end,
        ] {
            if let Err(e) = validate_date(d) {
                return format!("Error: {e}");
            }
        }
        if p.current_end < p.current_start {
            return "Error: current_end must be >= current_start".into();
        }
        if p.previous_end < p.previous_start {
            return "Error: previous_end must be >= previous_start".into();
        }
        let dims = p.dimensions.unwrap_or_else(|| vec!["query".into()]);
        if let Err(e) = validate_enum_list(&dims, VALID_DIMENSIONS, "dimension") {
            return format!("Error: {e}");
        }
        let limit = match validate_row_limit(p.row_limit, 50) {
            Ok(l) => l,
            Err(e) => return format!("Error: {e}"),
        };

        let current_body = build_analytics_body(
            &p.current_start,
            &p.current_end,
            &dims,
            "web",
            limit,
            0,
            "all",
            &[],
            None,
        );
        let previous_body = build_analytics_body(
            &p.previous_start,
            &p.previous_end,
            &dims,
            "web",
            limit,
            0,
            "all",
            &[],
            None,
        );

        let (current, previous) = tokio::join!(
            self.client
                .query_search_analytics(&p.site_url, &current_body),
            self.client
                .query_search_analytics(&p.site_url, &previous_body),
        );

        let current = match current {
            Ok(d) => d,
            Err(e) => return e.to_ui_string(),
        };
        let previous = match previous {
            Ok(d) => d,
            Err(e) => return e.to_ui_string(),
        };

        format_comparison(&current, &previous, &dims)
    }

    pub(crate) async fn handle_performance_overview(&self, p: PerformanceOverviewParams) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        let (start, end) = match resolve_dates(None, None, p.days, 28) {
            Ok(d) => d,
            Err(e) => return format!("Error: {e}"),
        };

        let agg_body = build_analytics_body(&start, &end, &[], "web", 1, 0, "all", &[], None);
        let trend_limit = p.days.unwrap_or(28).clamp(1, 540);
        let trend_body = build_analytics_body(
            &start,
            &end,
            &["date".into()],
            "web",
            trend_limit,
            0,
            "all",
            &[],
            None,
        );

        let (agg, trend) = tokio::join!(
            self.client.query_search_analytics(&p.site_url, &agg_body),
            self.client.query_search_analytics(&p.site_url, &trend_body),
        );

        let mut out = format!(
            "Performance Overview for {} ({start} to {end})\n\n",
            p.site_url
        );

        match agg {
            Ok(data) => {
                if let Some(rows) = data.get("rows").and_then(|r| r.as_array()) {
                    if let Some(row) = rows.first() {
                        let c = metric(row, "clicks");
                        let i = metric(row, "impressions");
                        let ctr = metric(row, "ctr");
                        let pos = metric(row, "position");
                        out.push_str(&format!("Total Clicks: {}\n", c as u64));
                        out.push_str(&format!("Total Impressions: {}\n", i as u64));
                        out.push_str(&format!("Average CTR: {:.2}%\n", ctr * 100.0));
                        out.push_str(&format!("Average Position: {pos:.1}\n"));
                    }
                } else {
                    out.push_str("No aggregate data available.\n");
                }
            }
            Err(e) => return e.to_ui_string(),
        }

        out.push_str("\nDaily Trend:\n");
        match trend {
            Ok(data) => {
                if let Some(rows) = data.get("rows").and_then(|r| r.as_array()) {
                    for row in rows {
                        let date = row_key(row, 0);
                        let date = if date.is_empty() { "?" } else { date };
                        let c = metric(row, "clicks");
                        let i = metric(row, "impressions");
                        out.push_str(&format!(
                            "  {date}: clicks={}, impressions={}\n",
                            c as u64, i as u64
                        ));
                    }
                }
            }
            Err(e) => out.push_str(&format!("  Error: {}\n", e.to_ui_string())),
        }

        out
    }

    pub(crate) async fn handle_page_query_breakdown(&self, p: PageQueryBreakdownParams) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
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
            dimension: "page".into(),
            operator: "equals".into(),
            expression: p.page_url.clone(),
        }];

        let body = build_analytics_body(
            &start,
            &end,
            &["query".into()],
            "web",
            limit,
            0,
            "all",
            &filters,
            None,
        );

        match self.client.query_search_analytics(&p.site_url, &body).await {
            Ok(data) => {
                let dims = vec!["query".into()];
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

fn format_comparison(
    current: &serde_json::Value,
    previous: &serde_json::Value,
    dims: &[String],
) -> String {
    let cur_rows = current.get("rows").and_then(|r| r.as_array());
    let prev_rows = previous.get("rows").and_then(|r| r.as_array());

    let mut out = String::from("Period Comparison\n\n");

    let (Some(cur_rows), Some(prev_rows)) = (cur_rows, prev_rows) else {
        return format!("{out}No data available for comparison.");
    };

    // Build lookup from previous period by keys
    let mut prev_map: std::collections::HashMap<String, &serde_json::Value> =
        std::collections::HashMap::with_capacity(prev_rows.len());
    for row in prev_rows {
        if let Some(keys) = row.get("keys").and_then(|k| k.as_array()) {
            let key_str = keys
                .iter()
                .map(|k| k.as_str().unwrap_or(""))
                .collect::<Vec<_>>()
                .join("|");
            prev_map.insert(key_str, row);
        }
    }

    let mut cur_keys = std::collections::HashSet::with_capacity(cur_rows.len());
    for row in cur_rows {
        let keys = row.get("keys").and_then(|k| k.as_array());
        if let Some(keys) = keys {
            let key_str = keys
                .iter()
                .map(|k| k.as_str().unwrap_or(""))
                .collect::<Vec<_>>()
                .join("|");
            cur_keys.insert(key_str.clone());
            for (i, k) in keys.iter().enumerate() {
                let name = dims.get(i).map_or("dim", std::string::String::as_str);
                out.push_str(&format!("{name}: {}  ", k.as_str().unwrap_or("?")));
            }

            let c1 = metric(row, "clicks");
            let i1 = metric(row, "impressions");
            let ctr1 = metric(row, "ctr");
            let pos1 = metric(row, "position");

            if let Some(prev) = prev_map.get(&key_str) {
                let c0 = metric(prev, "clicks");
                let i0 = metric(prev, "impressions");
                let ctr0 = metric(prev, "ctr");
                let pos0 = metric(prev, "position");
                let c_delta = c1 - c0;
                let i_delta = i1 - i0;
                let c_pct_str = if c0 > 0.0 {
                    format!("{:+.1}%", c_delta / c0 * 100.0)
                } else if c1 > 0.0 {
                    "new".into()
                } else {
                    "0%".into()
                };
                let i_pct_str = if i0 > 0.0 {
                    format!("{:+.1}%", i_delta / i0 * 100.0)
                } else if i1 > 0.0 {
                    "new".into()
                } else {
                    "0%".into()
                };
                let ctr_delta_pp = (ctr1 - ctr0) * 100.0;
                let pos_delta = pos1 - pos0;
                out.push_str(&format!(
                    "| clicks: {} ({:+.0}, {}) | impressions: {} ({:+.0}, {}) | ctr: {:.2}% ({:+.2}pp) | position: {:.1} ({:+.1})\n",
                    c1 as u64, c_delta, c_pct_str, i1 as u64, i_delta, i_pct_str,
                    ctr1 * 100.0, ctr_delta_pp, pos1, pos_delta
                ));
            } else {
                out.push_str(&format!(
                    "| clicks: {} (new) | impressions: {} (new) | ctr: {:.2}% (new) | position: {:.1} (new)\n",
                    c1 as u64, i1 as u64, ctr1 * 100.0, pos1
                ));
            }
        }
    }

    for row in prev_rows {
        let Some(keys) = row.get("keys").and_then(|k| k.as_array()) else {
            continue;
        };

        let key_str = keys
            .iter()
            .map(|k| k.as_str().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("|");

        if cur_keys.contains(&key_str) {
            continue;
        }

        for (i, k) in keys.iter().enumerate() {
            let name = dims.get(i).map_or("dim", std::string::String::as_str);
            out.push_str(&format!("{name}: {}  ", k.as_str().unwrap_or("?")));
        }

        let c0 = metric(row, "clicks");
        let i0 = metric(row, "impressions");
        let ctr0 = metric(row, "ctr");
        let pos0 = metric(row, "position");
        out.push_str(&format!(
            "| clicks: 0 ({:+.0}, {:+.1}%) | impressions: 0 ({:+.0}, {:+.1}%) | ctr: 0.00% ({:+.2}pp) | position: 0.0 ({:+.1})\n",
            -c0,
            -100.0,
            -i0,
            -100.0,
            -(ctr0 * 100.0),
            -pos0
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::{analytics_fetch_plan, format_comparison};

    #[test]
    fn explicit_sort_fetches_full_candidate_set() {
        let (fetch_limit, fetch_start_row) = analytics_fetch_plan(50, 100, Some("position"), None);

        assert_eq!(fetch_limit, 25_000);
        assert_eq!(fetch_start_row, 0);
    }

    #[test]
    fn compare_periods_includes_previous_only_rows() {
        let current = serde_json::json!({
            "rows": [
                {"keys": ["shared"], "clicks": 20.0, "impressions": 200.0, "ctr": 0.1, "position": 3.0}
            ]
        });
        let previous = serde_json::json!({
            "rows": [
                {"keys": ["shared"], "clicks": 10.0, "impressions": 100.0, "ctr": 0.1, "position": 4.0},
                {"keys": ["gone"], "clicks": 15.0, "impressions": 150.0, "ctr": 0.1, "position": 5.0}
            ]
        });

        let output = format_comparison(&current, &previous, &["query".into()]);

        assert!(output.contains("query: gone"));
    }
}
