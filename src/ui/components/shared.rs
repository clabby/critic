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

/// Formats a unix timestamp (ms) into a compact relative duration like "3d" or "2h".
pub fn short_timestamp(unix_ms: i64) -> String {
    if unix_ms <= 0 {
        return short_relative_duration(0);
    }

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or_default();
    let diff_ms = now_ms.saturating_sub(unix_ms);
    short_relative_duration(diff_ms)
}

fn short_relative_duration(diff_ms: i64) -> String {
    let seconds = diff_ms.max(0) / 1_000;
    if seconds < 60 {
        return compact_age(seconds, "s");
    }

    let minutes = seconds / 60;
    if minutes < 60 {
        return compact_age(minutes, "m");
    }

    let hours = minutes / 60;
    if hours < 24 {
        return compact_age(hours, "h");
    }

    let days = hours / 24;
    if days < 14 {
        return compact_age(days, "d");
    }

    let weeks = days / 7;
    if weeks < 8 {
        return compact_age(weeks, "w");
    }

    let months = days / 30;
    if months < 24 {
        return compact_age(months, "M");
    }

    let years = (days / 365).min(99);
    compact_age(years, "y")
}

fn compact_age(value: i64, unit: &str) -> String {
    format!("{value:>2}{unit:<2}")
}

#[cfg(test)]
mod tests {
    use super::short_relative_duration;

    #[test]
    fn formats_compact_relative_duration_units() {
        assert_eq!(short_relative_duration(10_000), "10s ");
        assert_eq!(short_relative_duration(120_000), " 2m ");
        assert_eq!(short_relative_duration(7_200_000), " 2h ");
        assert_eq!(short_relative_duration(86_400_000), " 1d ");
        assert_eq!(short_relative_duration(7 * 86_400_000), " 7d ");
        assert_eq!(short_relative_duration(100 * 86_400_000), " 3M ");
        assert_eq!(short_relative_duration(700 * 86_400_000), "23M ");
        assert_eq!(short_relative_duration(3_000 * 86_400_000), " 8y ");
    }

    #[test]
    fn respects_bucket_boundaries() {
        assert_eq!(short_relative_duration(59 * 60_000), "59m ");
        assert_eq!(short_relative_duration(60 * 60_000), " 1h ");
        assert_eq!(short_relative_duration(23 * 3_600_000), "23h ");
        assert_eq!(short_relative_duration(24 * 3_600_000), " 1d ");
        assert_eq!(short_relative_duration(13 * 86_400_000), "13d ");
        assert_eq!(short_relative_duration(14 * 86_400_000), " 2w ");
        assert_eq!(short_relative_duration(55 * 86_400_000), " 7w ");
        assert_eq!(short_relative_duration(56 * 86_400_000), " 1M ");
    }
}
