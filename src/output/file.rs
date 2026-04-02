use crate::types::metric;
use serde_json::Value;
use std::borrow::Cow;
use std::path::PathBuf;

fn output_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("GSC_MCP_OUTPUT_DIR") {
        PathBuf::from(dir)
    } else {
        std::env::temp_dir().join("gsc-mcp-rs")
    }
}

pub fn write_csv(
    data: &Value,
    rows_key: &str,
    filename: &str,
    dimensions: Option<&[String]>,
) -> Result<String, String> {
    let dir = output_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Cannot create output directory: {e}"))?;

    // Sanitize filename: reject path separators and traversal
    let safe_filename = filename.replace(['/', '\\'], "_").replace("..", "_");
    if safe_filename != filename {
        eprintln!(
            "[gsc-mcp-rs] Warning: filename sanitized from '{filename}' to '{safe_filename}'"
        );
    }

    let rows = data
        .get(rows_key)
        .and_then(|v| v.as_array())
        .ok_or("No rows found in data")?;

    if rows.is_empty() {
        return Ok("No data to export.".into());
    }

    // Determine headers
    let is_analytics = rows.first().and_then(|r| r.get("keys")).is_some();

    let headers: Vec<String> = if is_analytics {
        let key_count = rows
            .first()
            .and_then(|r| r.get("keys"))
            .and_then(|k| k.as_array())
            .map_or(0, std::vec::Vec::len);

        let mut h: Vec<String> = if let Some(dims) = dimensions {
            dims.iter().take(key_count).cloned().collect()
        } else {
            (0..key_count).map(|i| format!("dim_{i}")).collect()
        };
        // Pad if dimensions shorter than key_count
        while h.len() < key_count {
            h.push(format!("dim_{}", h.len()));
        }
        h.extend(
            ["clicks", "impressions", "ctr", "position"]
                .iter()
                .map(std::string::ToString::to_string),
        );
        h
    } else if let Some(obj) = rows.first().and_then(|r| r.as_object()) {
        obj.keys().cloned().collect()
    } else {
        return Err("Cannot determine CSV headers".into());
    };

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let filepath = dir.join(format!("{safe_filename}_{timestamp}.csv"));

    // Validate path stays within output dir BEFORE writing (no directory traversal)
    let canonical_dir =
        std::fs::canonicalize(&dir).map_err(|e| format!("Cannot resolve output dir: {e}"))?;
    // Verify parent of filepath is within canonical_dir
    if let Some(parent) = filepath.parent()
        && let Ok(canonical_parent) = std::fs::canonicalize(parent)
        && !canonical_parent.starts_with(&canonical_dir)
    {
        return Err("Security error: export path escaped output directory".into());
    }

    let mut csv_content = String::with_capacity(rows.len() * 100);
    csv_content.push_str(&headers.join(","));
    csv_content.push('\n');

    for row in rows {
        let mut values: Vec<String> = Vec::new();
        if is_analytics {
            if let Some(keys) = row.get("keys").and_then(|k| k.as_array()) {
                for key in keys {
                    values.push(csv_escape(key.as_str().unwrap_or(&key.to_string())).into_owned());
                }
            }
            values.push((metric(row, "clicks") as u64).to_string());
            values.push((metric(row, "impressions") as u64).to_string());
            values.push(format!("{:.6}", metric(row, "ctr")));
            values.push(format!("{:.1}", metric(row, "position")));
        } else if let Some(obj) = row.as_object() {
            for header in &headers {
                let val = obj
                    .get(header)
                    .map(|v| {
                        if let Some(s) = v.as_str() {
                            s.to_string()
                        } else {
                            v.to_string()
                        }
                    })
                    .unwrap_or_default();
                values.push(csv_escape(&val).into_owned());
            }
        }
        csv_content.push_str(&values.join(","));
        csv_content.push('\n');
    }

    std::fs::write(&filepath, &csv_content).map_err(|e| format!("Cannot write CSV file: {e}"))?;

    // Build summary
    let total_rows = rows.len();
    let mut total_clicks = 0f64;
    let mut total_impressions = 0f64;
    let mut sum_position = 0f64;
    let mut position_count = 0u64;

    for row in rows {
        total_clicks += metric(row, "clicks");
        total_impressions += metric(row, "impressions");
        let pos = metric(row, "position");
        if pos != 0.0 {
            sum_position += pos;
            position_count += 1;
        }
    }

    let avg_position = if position_count > 0 {
        sum_position / position_count as f64
    } else {
        0.0
    };

    Ok(format!(
        "Data exported to: {}\n\n\
         Summary:\n\
         - Rows: {}\n\
         - Total clicks: {}\n\
         - Total impressions: {}\n\
         - Average position: {:.1}\n\n\
         Use the file path above to access the full dataset.",
        filepath.display(),
        total_rows,
        total_clicks as u64,
        total_impressions as u64,
        avg_position
    ))
}

fn csv_escape(s: &str) -> Cow<'_, str> {
    // Defend against CSV formula injection (CWE-1236): prepend a single quote
    // to values starting with characters that spreadsheets interpret as formulas.
    let needs_formula_guard = s
        .as_bytes()
        .first()
        .is_some_and(|&b| b == b'=' || b == b'+' || b == b'-' || b == b'@' || b == b'\t');
    if needs_formula_guard {
        Cow::Owned(format!("\"'{}\"", s.replace('"', "\"\"")))
    } else if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        Cow::Owned(format!("\"{}\"", s.replace('"', "\"\"")))
    } else {
        Cow::Borrowed(s)
    }
}
