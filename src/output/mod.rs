pub mod file;
pub mod inline;

use serde_json::Value;

/// Default inline row limit. Above this, export to CSV file.
pub const DEFAULT_INLINE_LIMIT: u32 = 500;

/// Format API response based on row count vs inline_limit.
/// Returns the text to include in the MCP response.
pub fn format_response(
    data: &Value,
    rows_key: &str,
    inline_limit: u32,
    export_filename: Option<&str>,
    dimensions: Option<&[String]>,
) -> String {
    let rows = data
        .get(rows_key)
        .and_then(|v| v.as_array())
        .map_or(0, std::vec::Vec::len);

    if rows == 0 {
        return "No data found for the given parameters.".into();
    }

    if rows as u32 <= inline_limit {
        inline::format_inline(data, rows_key, dimensions)
    } else {
        let filename = export_filename.unwrap_or("gsc_export");
        match file::write_csv(data, rows_key, filename, dimensions) {
            Ok(info) => info,
            Err(e) => {
                // Fallback to inline if CSV write fails
                format!(
                    "Warning: CSV export failed ({e}). Showing inline instead.\n\n{}",
                    inline::format_inline(data, rows_key, dimensions)
                )
            }
        }
    }
}
