use crate::types::metric;
use serde_json::Value;

/// Format search analytics data as a readable inline response.
/// If dimensions are provided, uses them as column headers for the keys array.
pub fn format_inline(data: &Value, rows_key: &str, dimensions: Option<&[String]>) -> String {
    let Some(rows) = data.get(rows_key).and_then(|v| v.as_array()) else {
        return serde_json::to_string_pretty(data).unwrap_or_else(|e| format!("Error: {e}"));
    };

    // For search analytics rows (keys + metrics), format as readable table
    if rows.first().and_then(|r| r.get("keys")).is_some() {
        format_analytics_rows(rows, dimensions)
    } else {
        // Generic JSON for non-analytics responses
        serde_json::to_string_pretty(data).unwrap_or_else(|e| format!("Error: {e}"))
    }
}

fn format_analytics_rows(rows: &[Value], dimensions: Option<&[String]>) -> String {
    let mut out = String::new();

    // Summary line
    out.push_str(&format!("Results: {} rows\n\n", rows.len()));

    for row in rows {
        let keys = row.get("keys").and_then(|k| k.as_array());
        if let Some(keys) = keys {
            let dim_names = dimensions;
            for (i, key) in keys.iter().enumerate() {
                let name = dim_names
                    .and_then(|d| d.get(i))
                    .map_or("dim", std::string::String::as_str);
                let key_str = key.to_string();
                let val = key.as_str().unwrap_or(&key_str);
                out.push_str(&format!("{name}: {val}  "));
            }
        }

        let clicks = metric(row, "clicks");
        let impressions = metric(row, "impressions");
        let ctr = metric(row, "ctr");
        let position = metric(row, "position");

        out.push_str(&format!(
            "| clicks: {} | impressions: {} | ctr: {:.2}% | position: {:.1}\n",
            clicks as u64,
            impressions as u64,
            ctr * 100.0,
            position
        ));
    }

    out
}
