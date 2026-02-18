//! Shared component helpers.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Truncates multiline comment text to a compact single-line preview.
pub fn short_preview(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.len() <= max_chars {
        return normalized;
    }

    if max_chars <= 3 {
        return normalized.chars().take(max_chars).collect();
    }

    let mut out = String::new();
    for ch in normalized.chars().take(max_chars - 3) {
        out.push(ch);
    }
    out.push_str("...");
    out
}

/// Formats a unix timestamp (ms) into a compact relative duration like "3days" or "2h".
pub fn short_timestamp(unix_ms: i64) -> String {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or_default();

    let age_ms = (now_ms - unix_ms).max(0) as u64;
    let duration = Duration::from_millis(age_ms);
    let formatted = humantime::format_duration(duration).to_string();

    // Take only the most significant unit (first space-delimited token) + "ago".
    let unit = formatted.split_whitespace().next().unwrap_or("?");
    format!("{unit} ago")
}
