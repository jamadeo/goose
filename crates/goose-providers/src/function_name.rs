use once_cell::sync::Lazy;
use regex::Regex;

pub fn sanitize_function_name(name: &str) -> String {
    static INVALID_FUNCTION_NAME_CHARS: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"[^a-zA-Z0-9_-]").unwrap());
    INVALID_FUNCTION_NAME_CHARS
        .replace_all(name, "_")
        .to_string()
}

pub fn is_valid_function_name(name: &str) -> bool {
    static VALID_FUNCTION_NAME: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap());
    VALID_FUNCTION_NAME.is_match(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_function_name_replaces_invalid_characters() {
        assert_eq!(sanitize_function_name("hello-world"), "hello-world");
        assert_eq!(sanitize_function_name("hello world"), "hello_world");
        assert_eq!(sanitize_function_name("hello@world"), "hello_world");
    }

    #[test]
    fn valid_function_name_allows_only_provider_safe_characters() {
        assert!(is_valid_function_name("hello-world"));
        assert!(is_valid_function_name("hello_world"));
        assert!(!is_valid_function_name("hello world"));
        assert!(!is_valid_function_name("hello@world"));
    }
}
