use super::GscServer;
use super::analytics::{FULL_FETCH_LIMIT, sort_rows_by_metric};
use crate::output;
use crate::types::*;
use rmcp::schemars;
use serde::Deserialize;

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct TopPagesParams {
    pub site_url: String,
    pub days: Option<u32>,
    #[schemars(description = "Sort by: clicks, impressions, ctr, position. Default: clicks.")]
    pub sort_by: Option<String>,
    #[schemars(description = "Max rows. Default: 50.")]
    pub row_limit: Option<u32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct KeywordOpportunitiesParams {
    pub site_url: String,
    #[schemars(
        description = "Analysis mode. Default: 'quick_wins'. Valid: quick_wins (high impressions + low CTR), cannibalization (queries with 2+ competing pages), ctr_gaps (below-expected CTR for position), declining (biggest click losers vs previous period), growing (biggest click gainers)."
    )]
    pub mode: Option<String>,
    pub days: Option<u32>,
    #[schemars(
        description = "Min impressions to qualify. Default: 100. Used by quick_wins and ctr_gaps."
    )]
    pub min_impressions: Option<u32>,
    #[schemars(description = "Max CTR to qualify. Default: 0.03 (3%). Used by quick_wins only.")]
    pub max_ctr: Option<f64>,
    #[schemars(description = "Min position to qualify. Default: 4.0. Used by quick_wins only.")]
    pub position_range_min: Option<f64>,
    #[schemars(description = "Max position to qualify. Default: 20.0. Used by quick_wins only.")]
    pub position_range_max: Option<f64>,
    #[schemars(description = "Estimated value per click (for ROI). Used by quick_wins only.")]
    pub estimated_click_value: Option<f64>,
    #[schemars(description = "Estimated conversion rate (for ROI). Used by quick_wins only.")]
    pub conversion_rate: Option<f64>,
    pub row_limit: Option<u32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct BrandQueryParams {
    pub site_url: String,
    #[schemars(description = "Your brand terms, e.g. ['mycompany', 'mybrand'].")]
    pub brand_terms: Vec<String>,
    pub days: Option<u32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct KeywordTrendParams {
    pub site_url: String,
    #[schemars(description = "The keyword to track.")]
    pub keyword: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    #[schemars(description = "Days to look back. Default: 90.")]
    pub days: Option<u32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) struct DetectAnomaliesParams {
    pub site_url: String,
    #[schemars(description = "Recent period length in days. Default: 7.")]
    pub days: Option<u32>,
    pub dimensions: Option<Vec<String>>,
    #[schemars(description = "Critical threshold (fraction). Default: 0.5 (50% drop).")]
    pub drop_threshold_critical: Option<f64>,
    #[schemars(description = "Warning threshold (fraction). Default: 0.2 (20% drop).")]
    pub drop_threshold_warning: Option<f64>,
}

impl GscServer {
    pub(crate) async fn handle_top_pages(&self, p: TopPagesParams) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        let (start, end) = match resolve_dates(None, None, p.days, 28) {
            Ok(d) => d,
            Err(e) => return format!("Error: {e}"),
        };
        let limit = match validate_row_limit(p.row_limit, 50) {
            Ok(l) => l,
            Err(e) => return format!("Error: {e}"),
        };
        // Validate sort_by if provided
        let sort_by = p.sort_by.as_deref().unwrap_or("clicks");
        if let Err(e) = validate_enum(sort_by, VALID_METRICS, "sort_by") {
            return format!("Error: {e}");
        }
        let body = serde_json::json!({
            "startDate": start, "endDate": end,
            "dimensions": ["page"], "type": "web",
            "rowLimit": FULL_FETCH_LIMIT, "dataState": "all",
        });
        match self.client.query_search_analytics(&p.site_url, &body).await {
            Ok(mut data) => {
                // Client-side sorting - ascending for position (lower=better), descending for others
                let descending = sort_by != "position";
                if let Some(rows) = data.get_mut("rows").and_then(|v| v.as_array_mut()) {
                    sort_rows_by_metric(rows, sort_by, descending);
                    rows.truncate(limit as usize);
                }
                output::format_response(
                    &data,
                    "rows",
                    output::DEFAULT_INLINE_LIMIT,
                    None,
                    Some(&["page".into()]),
                )
            }
            Err(e) => e.to_ui_string(),
        }
    }

    pub(crate) async fn handle_keyword_opportunities(
        &self,
        p: KeywordOpportunitiesParams,
    ) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        let mode = p.mode.as_deref().unwrap_or("quick_wins");
        if let Err(e) = validate_enum(mode, VALID_OPPORTUNITY_MODES, "mode") {
            return format!("Error: {e}");
        }
        match mode {
            "quick_wins" => self.keyword_opps_quick_wins(&p).await,
            "cannibalization" => self.keyword_opps_cannibalization(&p).await,
            "ctr_gaps" => self.keyword_opps_ctr_gaps(&p).await,
            "declining" => self.keyword_opps_movers(&p, false).await,
            "growing" => self.keyword_opps_movers(&p, true).await,
            _ => unreachable!(),
        }
    }

    async fn keyword_opps_quick_wins(&self, p: &KeywordOpportunitiesParams) -> String {
        let (start, end) = match resolve_dates(None, None, p.days, 28) {
            Ok(d) => d,
            Err(e) => return format!("Error: {e}"),
        };
        let body = serde_json::json!({
            "startDate": start, "endDate": end,
            "dimensions": ["query"], "type": "web",
            "rowLimit": FULL_FETCH_LIMIT, "dataState": "all",
        });
        match self.client.query_search_analytics(&p.site_url, &body).await {
            Ok(data) => {
                let min_imp = p.min_impressions.unwrap_or(100) as f64;
                let max_ctr = p.max_ctr.unwrap_or(0.03);
                let pos_min = p.position_range_min.unwrap_or(4.0);
                let pos_max = p.position_range_max.unwrap_or(20.0);
                let limit = p.row_limit.unwrap_or(50) as usize;

                let Some(rows) = data.get("rows").and_then(|r| r.as_array()) else {
                    return "No keyword opportunities found matching the criteria.".into();
                };

                let mut opps: Vec<&serde_json::Value> = rows
                    .iter()
                    .filter(|row| {
                        let imp = metric(row, "impressions");
                        let ctr = metric(row, "ctr");
                        let pos = metric(row, "position");
                        let pos = if pos == 0.0 { 100.0 } else { pos };
                        imp >= min_imp && ctr <= max_ctr && pos >= pos_min && pos <= pos_max
                    })
                    .collect();
                opps.sort_by(|a, b| {
                    let ia = metric(a, "impressions");
                    let ib = metric(b, "impressions");
                    ib.partial_cmp(&ia).unwrap_or(std::cmp::Ordering::Equal)
                });
                opps.truncate(limit);

                if opps.is_empty() {
                    return "No keyword opportunities found matching the criteria.".into();
                }

                let mut out = format!("Keyword Opportunities ({} found)\n\n", opps.len());
                for row in &opps {
                    let q = row_key(row, 0);
                    let q = if q.is_empty() { "?" } else { q };
                    let c = metric(row, "clicks");
                    let i = metric(row, "impressions");
                    let ctr_val = metric(row, "ctr");
                    let pos = metric(row, "position");
                    out.push_str(&format!(
                        "  \"{q}\" - pos: {pos:.1}, impressions: {}, clicks: {}, ctr: {:.2}%",
                        i as u64,
                        c as u64,
                        ctr_val * 100.0
                    ));
                    if let (Some(cv), Some(cr)) = (p.estimated_click_value, p.conversion_rate) {
                        let potential_clicks = i * 0.1 - c;
                        let roi = potential_clicks.max(0.0) * cv * cr;
                        out.push_str(&format!(" | est. ROI: ${roi:.0}"));
                    }
                    out.push('\n');
                }
                out
            }
            Err(e) => e.to_ui_string(),
        }
    }

    async fn keyword_opps_cannibalization(&self, p: &KeywordOpportunitiesParams) -> String {
        let (start, end) = match resolve_dates(None, None, p.days, 28) {
            Ok(d) => d,
            Err(e) => return format!("Error: {e}"),
        };
        let body = serde_json::json!({
            "startDate": start, "endDate": end,
            "dimensions": ["query", "page"], "type": "web",
            "rowLimit": FULL_FETCH_LIMIT, "dataState": "all",
        });
        match self.client.query_search_analytics(&p.site_url, &body).await {
            Ok(data) => {
                let Some(rows) = data.get("rows").and_then(|r| r.as_array()) else {
                    return "No data found.".into();
                };
                let mut query_pages: std::collections::HashMap<
                    String,
                    Vec<(String, f64, f64, f64)>,
                > = std::collections::HashMap::with_capacity(rows.len());
                for row in rows {
                    let query = row_key(row, 0).to_string();
                    let page = row_key(row, 1).to_string();
                    if query.is_empty() {
                        continue;
                    }
                    let clicks = metric(row, "clicks");
                    let impressions = metric(row, "impressions");
                    let position = metric(row, "position");
                    query_pages.entry(query).or_default().push((
                        page,
                        clicks,
                        impressions,
                        position,
                    ));
                }
                let mut cannibalized: Vec<_> = query_pages
                    .iter()
                    .filter(|(_, pages)| pages.len() >= 2)
                    .collect();
                cannibalized.sort_by(|a, b| {
                    let ta: f64 = a.1.iter().map(|p| p.1).sum();
                    let tb: f64 = b.1.iter().map(|p| p.1).sum();
                    tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
                });
                let limit = p.row_limit.unwrap_or(20) as usize;
                cannibalized.truncate(limit);

                if cannibalized.is_empty() {
                    return "No keyword cannibalization detected.".into();
                }
                let mut out = format!(
                    "Keyword Cannibalization ({} queries with multiple ranking pages)\n\n",
                    cannibalized.len()
                );
                for (query, pages) in &cannibalized {
                    out.push_str(&format!("\"{}\" ({} pages):\n", query, pages.len()));
                    for (page, clicks, impressions, position) in *pages {
                        out.push_str(&format!(
                            "  {} - clicks: {}, impressions: {}, pos: {:.1}\n",
                            page, *clicks as u64, *impressions as u64, position
                        ));
                    }
                    out.push('\n');
                }
                out
            }
            Err(e) => e.to_ui_string(),
        }
    }

    async fn keyword_opps_ctr_gaps(&self, p: &KeywordOpportunitiesParams) -> String {
        let (start, end) = match resolve_dates(None, None, p.days, 28) {
            Ok(d) => d,
            Err(e) => return format!("Error: {e}"),
        };
        let min_imp = p.min_impressions.unwrap_or(100) as f64;
        let body = serde_json::json!({
            "startDate": start, "endDate": end,
            "dimensions": ["query"], "type": "web",
            "rowLimit": FULL_FETCH_LIMIT, "dataState": "all",
        });
        // Industry-average CTR by position bracket
        let expected_ctr = |pos: f64| -> f64 {
            if pos <= 1.5 {
                0.30
            } else if pos <= 2.5 {
                0.15
            } else if pos <= 3.5 {
                0.10
            } else if pos <= 5.0 {
                0.06
            } else if pos <= 10.0 {
                0.03
            } else {
                0.01
            }
        };
        match self.client.query_search_analytics(&p.site_url, &body).await {
            Ok(data) => {
                let Some(rows) = data.get("rows").and_then(|r| r.as_array()) else {
                    return "No data found.".into();
                };
                let mut gaps: Vec<_> = rows
                    .iter()
                    .filter(|row| {
                        let imp = metric(row, "impressions");
                        let ctr = metric(row, "ctr");
                        let pos = metric(row, "position");
                        let pos = if pos == 0.0 { 100.0 } else { pos };
                        imp >= min_imp && ctr < expected_ctr(pos) && pos <= 20.0
                    })
                    .collect();
                gaps.sort_by(|a, b| {
                    let ia = metric(a, "impressions");
                    let ib = metric(b, "impressions");
                    ib.partial_cmp(&ia).unwrap_or(std::cmp::Ordering::Equal)
                });
                let limit = p.row_limit.unwrap_or(50) as usize;
                gaps.truncate(limit);

                if gaps.is_empty() {
                    return "No CTR gap opportunities found.".into();
                }
                let mut out = format!("CTR Gap Opportunities ({} found)\n\n", gaps.len());
                for row in &gaps {
                    let q = row_key(row, 0);
                    let q = if q.is_empty() { "?" } else { q };
                    let imp = metric(row, "impressions");
                    let ctr = metric(row, "ctr");
                    let pos = metric(row, "position");
                    let exp = expected_ctr(pos);
                    out.push_str(&format!(
                        "  \"{}\" - pos: {:.1}, ctr: {:.2}% (expected: {:.0}%), impressions: {} | potential: +{} clicks\n",
                        q, pos, ctr * 100.0, exp * 100.0, imp as u64, ((exp - ctr) * imp) as i64
                    ));
                }
                out
            }
            Err(e) => e.to_ui_string(),
        }
    }

    async fn keyword_opps_movers(&self, p: &KeywordOpportunitiesParams, growing: bool) -> String {
        let days = p.days.unwrap_or(28);
        if !(1..=270).contains(&days) {
            return format!(
                "Error: days must be between 1 and 270 (comparison needs days*2 <= 540), got {days}"
            );
        }
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let ((cur_start, cur_end), (prev_start, prev_end)) =
            anomaly_date_ranges_from_now(now_secs, days);

        let cur_body = serde_json::json!({
            "startDate": cur_start, "endDate": cur_end,
            "dimensions": ["query"], "type": "web",
            "rowLimit": FULL_FETCH_LIMIT, "dataState": "all",
        });
        let prev_body = serde_json::json!({
            "startDate": prev_start, "endDate": prev_end,
            "dimensions": ["query"], "type": "web",
            "rowLimit": FULL_FETCH_LIMIT, "dataState": "all",
        });

        let (cur_result, prev_result) = tokio::join!(
            self.client.query_search_analytics(&p.site_url, &cur_body),
            self.client.query_search_analytics(&p.site_url, &prev_body),
        );
        let cur_data = match cur_result {
            Ok(d) => d,
            Err(e) => return e.to_ui_string(),
        };
        let prev_data = match prev_result {
            Ok(d) => d,
            Err(e) => return e.to_ui_string(),
        };

        let extract_map = |data: &serde_json::Value| -> std::collections::HashMap<String, f64> {
            let rows = data.get("rows").and_then(|r| r.as_array());
            let capacity = rows.map_or(0, Vec::len);
            let mut map = std::collections::HashMap::with_capacity(capacity);
            if let Some(rows) = rows {
                for row in rows {
                    let q = row_key(row, 0).to_string();
                    let clicks = metric(row, "clicks");
                    map.insert(q, clicks);
                }
            }
            map
        };
        let cur_map = extract_map(&cur_data);
        let prev_map = extract_map(&prev_data);

        let mut all_queries: std::collections::HashSet<String> =
            std::collections::HashSet::with_capacity(cur_map.len() + prev_map.len());
        for k in cur_map.keys() {
            all_queries.insert(k.clone());
        }
        for k in prev_map.keys() {
            all_queries.insert(k.clone());
        }

        let mut movers: Vec<(String, f64, f64, f64)> = Vec::new();
        for q in &all_queries {
            let cc = cur_map.get(q).copied().unwrap_or(0.0);
            let pc = prev_map.get(q).copied().unwrap_or(0.0);
            let delta = cc - pc;
            if (cc > 5.0 || pc > 5.0) && ((growing && delta > 0.0) || (!growing && delta < 0.0)) {
                movers.push((q.clone(), delta, cc, pc));
            }
        }

        if growing {
            movers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        } else {
            movers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        }
        let limit = p.row_limit.unwrap_or(50) as usize;
        movers.truncate(limit);

        let label = if growing { "Growing" } else { "Declining" };
        if movers.is_empty() {
            return format!("No {label} queries found in the last {days} days.");
        }
        let mut out = format!("{label} Queries ({} found)\n\n", movers.len());
        for (q, delta, cur, prev) in &movers {
            let pct = if *prev > 0.0 {
                format!("{:+.1}%", delta / prev * 100.0)
            } else {
                "new".into()
            };
            out.push_str(&format!(
                "  \"{}\" - clicks: {} -> {} ({:+.0}, {})\n",
                q, *prev as u64, *cur as u64, delta, pct
            ));
        }
        out
    }

    pub(crate) async fn handle_brand_query_analysis(&self, p: BrandQueryParams) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        if p.brand_terms.is_empty() || p.brand_terms.iter().all(|t| t.trim().is_empty()) {
            return "Error: brand_terms cannot be empty.".into();
        }
        let (start, end) = match resolve_dates(None, None, p.days, 28) {
            Ok(d) => d,
            Err(e) => return format!("Error: {e}"),
        };
        let body = serde_json::json!({
            "startDate": start, "endDate": end,
            "dimensions": ["query"], "type": "web",
            "rowLimit": FULL_FETCH_LIMIT, "dataState": "all",
        });
        match self.client.query_search_analytics(&p.site_url, &body).await {
            Ok(data) => {
                let Some(rows) = data.get("rows").and_then(|r| r.as_array()) else {
                    return "No data found.".into();
                };
                let brand_lower: Vec<String> = p
                    .brand_terms
                    .iter()
                    .filter(|t| !t.trim().is_empty())
                    .map(|t| t.to_lowercase())
                    .collect();
                let (mut bc, mut bi, mut nc, mut ni) = (0f64, 0f64, 0f64, 0f64);
                for row in rows {
                    let q = row_key(row, 0).to_lowercase();
                    let c = metric(row, "clicks");
                    let i = metric(row, "impressions");
                    if brand_lower.iter().any(|b| q.contains(b.as_str())) {
                        bc += c;
                        bi += i;
                    } else {
                        nc += c;
                        ni += i;
                    }
                }
                let total_c = bc + nc;
                let brand_pct = if total_c > 0.0 {
                    bc / total_c * 100.0
                } else {
                    0.0
                };
                format!(
                    "Brand vs Non-Brand Analysis\n\n\
                     Brand (terms: {:?}):\n  Clicks: {} | Impressions: {}\n\n\
                     Non-Brand:\n  Clicks: {} | Impressions: {}\n\n\
                     Brand share of clicks: {:.1}%",
                    p.brand_terms, bc as u64, bi as u64, nc as u64, ni as u64, brand_pct
                )
            }
            Err(e) => e.to_ui_string(),
        }
    }

    pub(crate) async fn handle_keyword_trend(&self, p: KeywordTrendParams) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        if p.keyword.trim().is_empty() {
            return "Error: keyword cannot be empty.".into();
        }
        let (start, end) =
            match resolve_dates(p.start_date.as_deref(), p.end_date.as_deref(), p.days, 90) {
                Ok(d) => d,
                Err(e) => return format!("Error: {e}"),
            };
        let filters = vec![Filter {
            dimension: "query".into(),
            operator: "equals".into(),
            expression: p.keyword.clone(),
        }];
        let body = serde_json::json!({
            "startDate": start, "endDate": end,
            "dimensions": ["date"], "type": "web",
            "rowLimit": 540, "dataState": "all",
            "dimensionFilterGroups": filters_to_groups(&filters),
        });
        match self.client.query_search_analytics(&p.site_url, &body).await {
            Ok(data) => {
                let dims = vec!["date".into()];
                let mut header = format!("Keyword Trend: \"{}\"\n\n", p.keyword);
                header.push_str(&output::format_response(
                    &data,
                    "rows",
                    output::DEFAULT_INLINE_LIMIT,
                    Some("keyword_trend"),
                    Some(&dims),
                ));
                header
            }
            Err(e) => e.to_ui_string(),
        }
    }

    pub(crate) async fn handle_detect_anomalies(&self, p: DetectAnomaliesParams) -> String {
        if let Err(e) = validate_site_url(&p.site_url) {
            return format!("Error: {e}");
        }
        let days = p.days.unwrap_or(7);
        if !(1..=270).contains(&days) {
            return format!(
                "Error: days must be between 1 and 270 (since the comparison period needs days*2 <= 540), got {days}"
            );
        }
        let dims = p.dimensions.unwrap_or_else(|| vec!["query".into()]);
        if let Err(e) = validate_enum_list(&dims, VALID_DIMENSIONS, "dimension") {
            return format!("Error: {e}");
        }
        let crit = p.drop_threshold_critical.unwrap_or(0.5);
        let warn = p.drop_threshold_warning.unwrap_or(0.2);
        if warn >= crit {
            return format!(
                "Error: drop_threshold_warning ({warn}) must be less than drop_threshold_critical ({crit})."
            );
        }

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let ((recent_start, recent_end), (prev_start, prev_end)) =
            anomaly_date_ranges_from_now(now_secs, days);

        let recent_body = serde_json::json!({
            "startDate": recent_start, "endDate": recent_end,
            "dimensions": dims, "type": "web",
            "rowLimit": FULL_FETCH_LIMIT, "dataState": "all",
        });
        let prev_body = serde_json::json!({
            "startDate": prev_start, "endDate": prev_end,
            "dimensions": dims, "type": "web",
            "rowLimit": FULL_FETCH_LIMIT, "dataState": "all",
        });

        let (recent, prev) = tokio::join!(
            self.client
                .query_search_analytics(&p.site_url, &recent_body),
            self.client.query_search_analytics(&p.site_url, &prev_body),
        );
        let recent = match recent {
            Ok(d) => d,
            Err(e) => return e.to_ui_string(),
        };
        let prev = match prev {
            Ok(d) => d,
            Err(e) => return e.to_ui_string(),
        };

        let recent_rows = recent.get("rows").and_then(|r| r.as_array());
        let prev_rows = prev.get("rows").and_then(|r| r.as_array());

        let (Some(recent_rows), Some(prev_rows)) = (recent_rows, prev_rows) else {
            return "Not enough data for anomaly detection.".into();
        };

        // prev_map stores (clicks, impressions, position)
        let mut prev_map: std::collections::HashMap<String, (f64, f64, f64)> =
            std::collections::HashMap::with_capacity(prev_rows.len());
        for row in prev_rows {
            let key = row
                .get("keys")
                .and_then(|k| k.as_array())
                .map(|a| {
                    a.iter()
                        .map(|v| v.as_str().unwrap_or(""))
                        .collect::<Vec<_>>()
                        .join("|")
                })
                .unwrap_or_default();
            let c = metric(row, "clicks");
            let i = metric(row, "impressions");
            let pos = metric(row, "position");
            prev_map.insert(key, (c, i, pos));
        }

        let mut anomalies = Vec::new();
        let mut seen_keys = std::collections::HashSet::with_capacity(recent_rows.len());
        for row in recent_rows {
            let key = row
                .get("keys")
                .and_then(|k| k.as_array())
                .map(|a| {
                    a.iter()
                        .map(|v| v.as_str().unwrap_or(""))
                        .collect::<Vec<_>>()
                        .join("|")
                })
                .unwrap_or_default();
            seen_keys.insert(key.clone());
            let c1 = metric(row, "clicks");
            let i1 = metric(row, "impressions");
            let pos1 = metric(row, "position");

            if let Some(&(c0, i0, pos0)) = prev_map.get(&key) {
                if c0 > 5.0 {
                    let drop = (c0 - c1) / c0;
                    if drop >= crit {
                        anomalies.push(format!(
                            "CRITICAL: \"{}\" - clicks dropped {:.0}% ({} -> {})",
                            key,
                            drop * 100.0,
                            c0 as u64,
                            c1 as u64
                        ));
                    } else if drop >= warn {
                        anomalies.push(format!(
                            "WARNING: \"{}\" - clicks dropped {:.0}% ({} -> {})",
                            key,
                            drop * 100.0,
                            c0 as u64,
                            c1 as u64
                        ));
                    }
                }
                if i0 > 50.0 {
                    let i_drop = (i0 - i1) / i0;
                    if i_drop >= crit {
                        anomalies.push(format!(
                            "CRITICAL: \"{}\" - impressions dropped {:.0}% ({} -> {})",
                            key,
                            i_drop * 100.0,
                            i0 as u64,
                            i1 as u64
                        ));
                    } else if i_drop >= warn {
                        anomalies.push(format!(
                            "WARNING: \"{}\" - impressions dropped {:.0}% ({} -> {})",
                            key,
                            i_drop * 100.0,
                            i0 as u64,
                            i1 as u64
                        ));
                    }
                }
                if pos0 > 0.0 && pos1 > 0.0 {
                    let pos_increase = pos1 - pos0;
                    if pos_increase >= 5.0 && pos0 <= 20.0 {
                        anomalies.push(format!("WARNING: \"{key}\" - position regressed from {pos0:.1} to {pos1:.1} ({pos_increase:+.1} spots)"));
                    }
                }
            }
        }

        // Flag keywords that disappeared entirely (100% drop)
        for (key, &(c0, _i0, _pos0)) in &prev_map {
            if !seen_keys.contains(key) && c0 > 5.0 {
                anomalies.push(format!(
                    "CRITICAL: \"{}\" - completely disappeared (was {} clicks, now absent)",
                    key, c0 as u64
                ));
            }
        }

        if anomalies.is_empty() {
            format!(
                "No anomalies detected in the last {days} days (thresholds: warning={:.0}%, critical={:.0}%).",
                warn * 100.0,
                crit * 100.0
            )
        } else {
            let mut out = format!("Anomalies Detected ({} issues)\n\n", anomalies.len());
            for a in &anomalies {
                out.push_str(a);
                out.push('\n');
            }
            out
        }
    }
}

fn anomaly_date_ranges_from_now(now_secs: u64, days: u32) -> ((String, String), (String, String)) {
    let end_secs = now_secs.saturating_sub(86400 * 2); // 2-day GSC data lag
    let recent_start_secs = end_secs.saturating_sub(days.saturating_sub(1) as u64 * 86400);
    let prev_end_secs = recent_start_secs.saturating_sub(86400); // one day before recent_start
    let prev_start_secs = prev_end_secs.saturating_sub(days.saturating_sub(1) as u64 * 86400);

    let to_date = |secs: u64| -> String {
        let ds = (secs / 86400) as i64;
        let (y, m, d) = crate::types::civil_from_days(ds);
        format!("{y:04}-{m:02}-{d:02}")
    };

    (
        (to_date(recent_start_secs), to_date(end_secs)),
        (to_date(prev_start_secs), to_date(prev_end_secs)),
    )
}

#[cfg(test)]
mod tests {
    use super::anomaly_date_ranges_from_now;

    fn date_to_days(date: &str) -> i64 {
        let parts: Vec<&str> = date.split('-').collect();
        let year: i64 = parts[0].parse().unwrap();
        let month: i64 = parts[1].parse().unwrap();
        let day: i64 = parts[2].parse().unwrap();

        let year = year - i64::from(month <= 2);
        let era = if year >= 0 { year } else { year - 399 } / 400;
        let yoe = year - era * 400;
        let mp = month + if month > 2 { -3 } else { 9 };
        let doy = (153 * mp + 2) / 5 + day - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }

    #[test]
    fn anomaly_periods_use_equal_inclusive_day_counts() {
        let now_secs = 30 * 86400;
        let ((recent_start, recent_end), (prev_start, prev_end)) =
            anomaly_date_ranges_from_now(now_secs, 7);
        let recent_days = date_to_days(&recent_end) - date_to_days(&recent_start) + 1;
        let previous_days = date_to_days(&prev_end) - date_to_days(&prev_start) + 1;

        assert_eq!(recent_days, 7);
        assert_eq!(previous_days, 7);
    }
}
