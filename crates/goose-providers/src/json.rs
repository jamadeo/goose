use serde_json::Value;

pub fn unescape_json_values(value: &Value) -> Value {
    let mut cloned = value.clone();
    unescape_json_values_in_place(&mut cloned);
    cloned
}

fn unescape_json_values_in_place(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for value in map.values_mut() {
                unescape_json_values_in_place(value);
            }
        }
        Value::Array(values) => {
            for value in values.iter_mut() {
                unescape_json_values_in_place(value);
            }
        }
        Value::String(text) => {
            if text.contains('\\') {
                *text = text
                    .replace("\\\\n", "\n")
                    .replace("\\\\t", "\t")
                    .replace("\\\\r", "\r")
                    .replace("\\\\\"", "\"")
                    .replace("\\n", "\n")
                    .replace("\\t", "\t")
                    .replace("\\r", "\r")
                    .replace("\\\"", "\"");
            }
        }
        _ => {}
    }
}

pub fn safely_parse_json(input: &str) -> Result<Value, serde_json::Error> {
    match serde_json::from_str(input) {
        Ok(value) => Ok(value),
        Err(_) => {
            for candidate in [
                repair_truncated_json(input),
                json_escape_control_chars_in_string(input),
            ] {
                if let Ok(value) = serde_json::from_str(&candidate) {
                    return Ok(value);
                }
            }

            let repaired = repair_truncated_json(&json_escape_control_chars_in_string(input));
            serde_json::from_str(&repaired)
        }
    }
}

fn repair_truncated_json(input: &str) -> String {
    let mut repaired = String::with_capacity(input.len() + 8);
    let mut in_string = false;
    let mut escape_next = false;
    let mut closers = Vec::new();

    for character in input.chars() {
        repaired.push(character);

        if in_string {
            if escape_next {
                escape_next = false;
                continue;
            }

            match character {
                '\\' => escape_next = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            '{' => closers.push('}'),
            '[' => closers.push(']'),
            '}' | ']' => {
                if closers.last() == Some(&character) {
                    closers.pop();
                }
            }
            _ => {}
        }
    }

    if in_string {
        if escape_next {
            repaired.push('\\');
        }
        repaired.push('"');
    }

    while let Some(closer) = closers.pop() {
        repaired.push(closer);
    }

    repaired
}

pub fn json_escape_control_chars_in_string(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for character in input.chars() {
        match character {
            '\u{0000}'..='\u{001F}' => match character {
                '\u{0008}' => escaped.push_str("\\b"),
                '\u{000C}' => escaped.push_str("\\f"),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\t' => escaped.push_str("\\t"),
                _ => {
                    escaped.push_str(&format!("\\u{:04x}", character as u32));
                }
            },
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unescape_json_values_with_object() {
        let value = json!({"text": "Hello\\nWorld"});
        let unescaped_value = unescape_json_values(&value);
        assert_eq!(unescaped_value, json!({"text": "Hello\nWorld"}));
    }

    #[test]
    fn unescape_json_values_with_array() {
        let value = json!(["Hello\\nWorld", "Goodbye\\tWorld"]);
        let unescaped_value = unescape_json_values(&value);
        assert_eq!(unescaped_value, json!(["Hello\nWorld", "Goodbye\tWorld"]));
    }

    #[test]
    fn unescape_json_values_with_string() {
        let value = json!("Hello\\nWorld");
        let unescaped_value = unescape_json_values(&value);
        assert_eq!(unescaped_value, json!("Hello\nWorld"));
    }

    #[test]
    fn unescape_json_values_with_mixed_content() {
        let value = json!({
            "text": "Hello\\nWorld\\\\n!",
            "array": ["Goodbye\\tWorld", "See you\\rlater"],
            "nested": {
                "inner_text": "Inner\\\"Quote\\\""
            }
        });
        let unescaped_value = unescape_json_values(&value);
        assert_eq!(
            unescaped_value,
            json!({
                "text": "Hello\nWorld\n!",
                "array": ["Goodbye\tWorld", "See you\rlater"],
                "nested": {
                    "inner_text": "Inner\"Quote\""
                }
            })
        );
    }

    #[test]
    fn unescape_json_values_with_no_escapes() {
        let value = json!({"text": "Hello World"});
        let unescaped_value = unescape_json_values(&value);
        assert_eq!(unescaped_value, json!({"text": "Hello World"}));
    }

    #[test]
    fn safely_parse_json_repairs_common_malformed_json() {
        let valid_json = r#"{"key1": "value1","key2": "value2"}"#;
        let result = safely_parse_json(valid_json).unwrap();
        assert_eq!(result["key1"], "value1");
        assert_eq!(result["key2"], "value2");

        let invalid_json = "{\"key1\": \"value1\n\",\"key2\": \"value2\"}";
        let result = safely_parse_json(invalid_json).unwrap();
        assert_eq!(result["key1"], "value1\n");
        assert_eq!(result["key2"], "value2");

        let good_json = r#"{"test": "value"}"#;
        let result = safely_parse_json(good_json).unwrap();
        assert_eq!(result["test"], "value");

        let truncated_json = r#"{"key": "unclosed_string","nested": {"items": [1, 2, 3"#;
        let result = safely_parse_json(truncated_json).unwrap();
        assert_eq!(result["key"], "unclosed_string");
        assert_eq!(result["nested"]["items"], json!([1, 2, 3]));

        let dangling_escape_json = String::from(r#"{"path":"abc\"#);
        let result = safely_parse_json(&dangling_escape_json).unwrap();
        assert_eq!(result["path"], "abc\\");

        let empty_json = "{}";
        let result = safely_parse_json(empty_json).unwrap();
        assert!(result.as_object().unwrap().is_empty());

        let escaped_json = r#"{"key": "value with\nnewline"}"#;
        let result = safely_parse_json(escaped_json).unwrap();
        assert_eq!(result["key"], "value with\nnewline");
    }

    #[test]
    fn json_escape_control_chars_in_string_escapes_control_characters() {
        assert_eq!(
            json_escape_control_chars_in_string("Hello\nWorld"),
            "Hello\\nWorld"
        );
        assert_eq!(
            json_escape_control_chars_in_string("Hello\tWorld"),
            "Hello\\tWorld"
        );
        assert_eq!(
            json_escape_control_chars_in_string("Hello\rWorld"),
            "Hello\\rWorld"
        );
        assert_eq!(
            json_escape_control_chars_in_string("Hello\n\tWorld\r"),
            "Hello\\n\\tWorld\\r"
        );
        assert_eq!(
            json_escape_control_chars_in_string("Hello \"World\""),
            "Hello \"World\""
        );
        assert_eq!(
            json_escape_control_chars_in_string("Hello\\World"),
            "Hello\\World"
        );
        assert_eq!(
            json_escape_control_chars_in_string("{\"message\": \"Hello\nWorld\"}"),
            "{\"message\": \"Hello\\nWorld\"}"
        );
        assert_eq!(
            json_escape_control_chars_in_string("Hello World"),
            "Hello World"
        );
        assert_eq!(
            json_escape_control_chars_in_string("Hello\u{0001}World"),
            "Hello\\u0001World"
        );
    }
}
