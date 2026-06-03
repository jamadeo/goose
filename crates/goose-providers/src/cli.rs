use crate::errors::ProviderError;
use goose_types::Usage;
use serde_json::Value;

pub const SESSION_NAME_BEGIN_MARKER: &str = "---BEGIN USER MESSAGES---";
pub const SESSION_NAME_END_MARKER: &str = "---END USER MESSAGES---";
pub const SESSION_NAME_SUFFIX: &str = "Generate a short title for the above messages.";

pub fn extract_usage_tokens(usage_info: &Value) -> Usage {
    let get = |key: &str| {
        usage_info
            .get(key)
            .and_then(|value| value.as_i64())
            .and_then(|value| i32::try_from(value).ok())
    };
    Usage::new(
        get("input_tokens"),
        get("output_tokens"),
        get("total_tokens"),
    )
}

pub fn error_from_event(provider_name: &str, parsed: &Value) -> ProviderError {
    let error_msg = parsed
        .get("error")
        .and_then(|error| error.as_str())
        .or_else(|| parsed.get("message").and_then(|message| message.as_str()))
        .unwrap_or("Unknown error");
    if error_msg.contains("context window exceeded") {
        ProviderError::ContextLengthExceeded(error_msg.to_string())
    } else {
        ProviderError::RequestFailed(format!("{provider_name} error: {error_msg}"))
    }
}

pub fn is_session_description_request(system: &str) -> bool {
    system.contains("four words or less") || system.contains("4 words or less")
}

pub fn strip_session_name_prompt_wrapper(text: &str) -> &str {
    let text = text
        .rfind(SESSION_NAME_BEGIN_MARKER)
        .and_then(|idx| text.get(idx..))
        .unwrap_or(text);
    let stripped = text
        .strip_prefix(SESSION_NAME_BEGIN_MARKER)
        .unwrap_or(text)
        .trim_start_matches(['\n', '\r']);
    let full_suffix = format!("{}\n\n{}", SESSION_NAME_END_MARKER, SESSION_NAME_SUFFIX);
    stripped
        .strip_suffix(&full_suffix)
        .or_else(|| stripped.strip_suffix(SESSION_NAME_END_MARKER))
        .unwrap_or(stripped)
        .trim()
}

pub fn simple_session_description_from_text(text: &str) -> String {
    let stripped = strip_session_name_prompt_wrapper(text);
    let description = stripped
        .split_whitespace()
        .take(4)
        .collect::<Vec<_>>()
        .join(" ");
    if description.is_empty() {
        "Simple task".to_string()
    } else {
        safe_truncate(&description, 100)
    }
}

fn safe_truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_chars.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_usage_tokens() {
        let usage = extract_usage_tokens(&json!({
            "input_tokens": 10,
            "output_tokens": 20,
            "total_tokens": 30,
            "ignored": 40,
        }));
        assert_eq!(usage.input_tokens, Some(10));
        assert_eq!(usage.output_tokens, Some(20));
        assert_eq!(usage.total_tokens, Some(30));
    }

    #[test]
    fn event_error_maps_context_window_errors() {
        let error = error_from_event(
            "Claude CLI",
            &json!({"error": "context window exceeded: too many tokens"}),
        );
        assert!(matches!(error, ProviderError::ContextLengthExceeded(_)));
    }

    #[test]
    fn event_error_maps_generic_errors() {
        let error = error_from_event("Claude CLI", &json!({"message": "boom"}));
        assert_eq!(
            error,
            ProviderError::RequestFailed("Claude CLI error: boom".to_string())
        );
    }

    #[test]
    fn detects_session_description_requests() {
        assert!(is_session_description_request(
            "Please use four words or less"
        ));
        assert!(is_session_description_request("Please use 4 words or less"));
        assert!(!is_session_description_request(
            "Please summarize this session"
        ));
    }

    #[test]
    fn strips_session_name_prompt_wrapper() {
        let text = format!(
            "{}\nList files in the repo\n{}\n\n{}",
            SESSION_NAME_BEGIN_MARKER, SESSION_NAME_END_MARKER, SESSION_NAME_SUFFIX
        );
        assert_eq!(
            strip_session_name_prompt_wrapper(&text),
            "List files in the repo"
        );
    }

    #[test]
    fn simple_session_description_uses_first_four_words() {
        let text = format!(
            "{}\nList files in the repo please\n{}\n\n{}",
            SESSION_NAME_BEGIN_MARKER, SESSION_NAME_END_MARKER, SESSION_NAME_SUFFIX
        );
        assert_eq!(
            simple_session_description_from_text(&text),
            "List files in the"
        );
    }

    #[test]
    fn simple_session_description_defaults_when_empty() {
        assert_eq!(simple_session_description_from_text(""), "Simple task");
    }
}
