//! Shared component helpers.

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
pub fn short_timestamp(value: &str) -> String {
    if value.len() >= 16 {
        return value[..16].replace('T', " ");
    }
    value.to_owned()
}
