use rmcp::schemars;
use serde::Deserialize;

pub(crate) const VALID_DIMENSIONS: &[&str] = &[
    "query",
    "page",
    "country",
    "device",
    "searchAppearance",
    "date",
    "hour",
];
pub(crate) const VALID_METRICS: &[&str] = &["clicks", "impressions", "ctr", "position"];
pub(crate) const VALID_SEARCH_TYPES: &[&str] =
    &["web", "image", "video", "news", "discover", "googleNews"];
pub(crate) const VALID_AGGREGATION_TYPES: &[&str] =
    &["auto", "byProperty", "byPage", "byNewsShowcasePanel"];
pub(crate) const VALID_DATA_STATES: &[&str] = &["all", "final", "hourly_all"];
pub(crate) const VALID_OPPORTUNITY_MODES: &[&str] = &[
    "quick_wins",
    "cannibalization",
    "ctr_gaps",
    "declining",
    "growing",
];
pub(crate) const VALID_SITE_ACTIONS: &[&str] = &["add", "delete"];
pub(crate) const VALID_SITEMAP_ACTIONS: &[&str] = &["submit", "delete"];
pub(crate) const VALID_FILTER_OPERATORS: &[&str] = &[
    "contains",
    "equals",
    "notContains",
    "notEquals",
    "includingRegex",
    "excludingRegex",
];
pub(crate) const VALID_SEARCH_APPEARANCES: &[&str] = &[
    "AMP_BLUE_LINK",
    "AMP_TOP_STORIES",
    "BREADCRUMB",
    "EVENT",
    "FAQ",
    "HOWTO",
    "IMAGE_PACK",
    "JOB_LISTING",
    "MERCHANT_LISTINGS",
    "PRODUCT_SNIPPETS",
    "RECIPE_FEATURE",
    "RECIPE_RICH_SNIPPET",
    "REVIEW_SNIPPET",
    "SITELINKS",
    "VIDEO",
    "WEB_STORY",
];
pub(crate) const VALID_NOTIFICATION_TYPES: &[&str] = &["URL_UPDATED", "URL_DELETED"];

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
pub struct Filter {
    pub dimension: String,
    pub operator: String,
    pub expression: String,
}

/// Validate site_url format
pub(crate) fn validate_site_url(site_url: &str) -> Result<(), String> {
    if site_url.starts_with("sc-domain:") {
        if site_url.len() <= 10 {
            return Err(
                "Invalid site_url: sc-domain: must be followed by a domain (e.g., 'sc-domain:example.com')".into(),
            );
        }
        Ok(())
    } else if site_url.starts_with("http://") || site_url.starts_with("https://") {
        Ok(())
    } else {
        Err("Invalid site_url format. Use 'https://example.com/' or 'sc-domain:example.com'".into())
    }
}

/// Validate date format YYYY-MM-DD
pub(crate) fn validate_date(date: &str) -> Result<(), String> {
    if date.len() != 10 {
        return Err(format!(
            "Invalid date '{date}'. Use YYYY-MM-DD format (e.g., '2026-03-01')."
        ));
    }
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        return Err(format!("Invalid date '{date}'. Use YYYY-MM-DD format."));
    }
    let y: u32 = parts[0]
        .parse()
        .map_err(|_| format!("Invalid year in '{date}'"))?;
    let m: u32 = parts[1]
        .parse()
        .map_err(|_| format!("Invalid month in '{date}'"))?;
    let d: u32 = parts[2]
        .parse()
        .map_err(|_| format!("Invalid day in '{date}'"))?;
    if !(2000..=2100).contains(&y) {
        return Err(format!("Year {y} out of range"));
    }
    if !(1..=12).contains(&m) {
        return Err(format!("Month {m} out of range (1-12)"));
    }
    let max_day = match m {
        2 => {
            if y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400)) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if d < 1 || d > max_day {
        return Err(format!("Day {d} out of range (1-{max_day}) for {y}-{m:02}"));
    }
    Ok(())
}

/// Validate a value against a list of valid options
pub(crate) fn validate_enum(value: &str, valid: &[&str], field_name: &str) -> Result<(), String> {
    if valid.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "Unknown {field_name} '{value}'. Valid: {}",
            valid.join(", ")
        ))
    }
}

/// Validate each item in a list against valid options
pub(crate) fn validate_enum_list(
    values: &[String],
    valid: &[&str],
    field_name: &str,
) -> Result<(), String> {
    for v in values {
        validate_enum(v, valid, field_name)?;
    }
    Ok(())
}

/// Calculate start_date and end_date from days parameter
pub(crate) fn days_to_date_range(days: u32) -> (String, String) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let end_secs = now.saturating_sub(86400 * 2); // 2-day lag for GSC data
    let start_secs = end_secs.saturating_sub(days.saturating_sub(1) as u64 * 86400);
    (secs_to_date(start_secs), secs_to_date(end_secs))
}

fn secs_to_date(secs: u64) -> String {
    let days_since_epoch = (secs / 86400) as i64;
    let (y, m, d) = civil_from_days(days_since_epoch);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Howard Hinnant's civil_from_days algorithm.
pub(crate) fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64 + era * 400) as i32;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Resolve date range from either explicit dates or days parameter.
pub(crate) fn resolve_dates(
    start_date: Option<&str>,
    end_date: Option<&str>,
    days: Option<u32>,
    default_days: u32,
) -> Result<(String, String), String> {
    match (start_date, end_date, days) {
        (Some(_), _, Some(_)) | (_, Some(_), Some(_)) => {
            Err("Use either 'days' or 'start_date'/'end_date', not both.".into())
        }
        (Some(s), Some(e), None) => {
            validate_date(s)?;
            validate_date(e)?;
            if e < s {
                return Err(format!("end_date '{e}' must be >= start_date '{s}'"));
            }
            Ok((s.to_string(), e.to_string()))
        }
        (Some(s), None, None) => {
            validate_date(s)?;
            let (_, default_end) = days_to_date_range(1);
            Ok((s.to_string(), default_end))
        }
        (None, Some(_), None) => {
            Err("If end_date is provided, start_date is also required.".into())
        }
        (None, None, Some(d)) => {
            if !(1..=540).contains(&d) {
                return Err("days must be between 1 and 540".into());
            }
            Ok(days_to_date_range(d))
        }
        (None, None, None) => Ok(days_to_date_range(default_days)),
    }
}

/// Validate filters array
pub(crate) fn validate_filters(filters: &[Filter]) -> Result<(), String> {
    for (i, f) in filters.iter().enumerate() {
        validate_enum(
            &f.dimension,
            VALID_DIMENSIONS,
            &format!("filter[{i}].dimension"),
        )?;
        validate_enum(
            &f.operator,
            VALID_FILTER_OPERATORS,
            &format!("filter[{i}].operator"),
        )?;
        if f.expression.is_empty() {
            return Err(format!("filter[{i}].expression cannot be empty"));
        }
    }
    Ok(())
}

/// Convert flat filters to Google's dimensionFilterGroups format
pub(crate) fn filters_to_groups(filters: &[Filter]) -> serde_json::Value {
    if filters.is_empty() {
        return serde_json::json!([]);
    }
    serde_json::json!([{
        "groupType": "and",
        "filters": filters.iter().map(|f| serde_json::json!({
            "dimension": f.dimension,
            "operator": f.operator,
            "expression": f.expression,
        })).collect::<Vec<_>>()
    }])
}

/// Extract a numeric metric from a JSON row.
pub(crate) fn metric(row: &serde_json::Value, key: &str) -> f64 {
    row.get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0)
}

/// Extract a key string from a JSON row's `keys` array by index.
pub(crate) fn row_key(row: &serde_json::Value, index: usize) -> &str {
    row.get("keys")
        .and_then(|k| k.as_array())
        .and_then(|a| a.get(index))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

/// Validate and resolve a row_limit parameter.
pub(crate) fn validate_row_limit(limit: Option<u32>, default: u32) -> Result<u32, String> {
    match limit {
        Some(l) if !(1..=25000).contains(&l) => {
            Err(format!("row_limit must be between 1 and 25000, got {l}"))
        }
        Some(l) => Ok(l),
        None => Ok(default),
    }
}

/// Minimal URL encoding for OAuth parameters and API paths (ASCII-safe inputs assumed).
pub(crate) fn urlencode(s: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut result = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            _ => {
                result.push('%');
                result.push(HEX[(b >> 4) as usize] as char);
                result.push(HEX[(b & 0x0F) as usize] as char);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::days_to_date_range;

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
    fn days_to_date_range_returns_exact_number_of_inclusive_days() {
        let (start, end) = days_to_date_range(7);
        let inclusive_days = date_to_days(&end) - date_to_days(&start) + 1;
        assert_eq!(inclusive_days, 7);
    }
}
