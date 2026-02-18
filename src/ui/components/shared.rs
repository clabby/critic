//! Shared component helpers.

use std::time::{SystemTime, UNIX_EPOCH};

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

/// Formats an ISO 8601 timestamp into a compact local-ish display format.
pub fn short_timestamp(value: &str, unix_ms: i64) -> String {
    const DAY_MS: i64 = 86_400_000;
    const YEAR_DAYS: i64 = 365;

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or_default();

    if now_ms.div_euclid(DAY_MS) == unix_ms.div_euclid(DAY_MS)
        && let Some(time) = extract_time(value)
    {
        return time;
    }

    let age_days = (now_ms - unix_ms).max(0).div_euclid(DAY_MS);
    if age_days < YEAR_DAYS {
        if let Some(date) = extract_month_day(value) {
            return date;
        }
    } else {
        let years = (age_days / YEAR_DAYS).clamp(1, 9);
        return format!(">{years} yr");
    }

    if let Some(date) = extract_month_day(value) {
        return date;
    }

    value.to_owned()
}

fn extract_month_day(value: &str) -> Option<String> {
    let month_tens = value.chars().nth(5)?;
    let month_ones = value.chars().nth(6)?;
    let day_tens = value.chars().nth(8)?;
    let day_ones = value.chars().nth(9)?;

    Some(format!("{month_tens}{month_ones}/{day_tens}{day_ones}"))
}

fn extract_time(value: &str) -> Option<String> {
    let time_start = value.find('T')?.saturating_add(1);
    let mut out = String::new();
    for ch in value.chars().skip(time_start).take(5) {
        out.push(ch);
    }

    if out.chars().count() == 5 && out.chars().nth(2) == Some(':') {
        Some(out)
    } else {
        None
    }
}
