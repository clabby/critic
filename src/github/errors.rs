//! Shared GitHub API error formatting helpers.

use std::error::Error as StdError;

/// Formats an octocrab error into a concise user-facing string.
pub fn format_octocrab_error(error: octocrab::Error) -> String {
    match error {
        octocrab::Error::GitHub { source, .. } => format_github_error(*source),
        other => format_error_chain(&other),
    }
}

fn format_github_error(error: octocrab::GitHubError) -> String {
    let mut message = format!(
        "status {}: {}",
        error.status_code.as_u16(),
        normalize_message(&error.message)
    );

    if let Some(url) = error.documentation_url {
        message.push_str(&format!(" | docs: {url}"));
    }

    if let Some(errors) = error.errors.filter(|errors| !errors.is_empty()) {
        let details = errors
            .iter()
            .map(format_github_detail)
            .collect::<Vec<_>>()
            .join("; ");
        if !details.is_empty() {
            message.push_str(&format!(" | details: {details}"));
        }
    }

    message
}

fn format_github_detail(value: &serde_json::Value) -> String {
    if let Some(message) = value.get("message").and_then(serde_json::Value::as_str) {
        return message.to_owned();
    }

    if let Some(text) = value.as_str() {
        return text.to_owned();
    }

    value.to_string()
}

fn normalize_message(message: &str) -> &str {
    let trimmed = message.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("github") {
        "GitHub API error"
    } else {
        trimmed
    }
}

fn format_error_chain(error: &(dyn StdError + 'static)) -> String {
    let mut current = Some(error);
    let mut parts = Vec::new();

    while let Some(err) = current {
        let text = err.to_string();
        if !text.is_empty() && parts.last() != Some(&text) {
            parts.push(text);
        }
        current = err.source();
    }

    if parts.is_empty() {
        "unknown error".to_owned()
    } else {
        parts.join(": ")
    }
}
