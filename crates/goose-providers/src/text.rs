use once_cell::sync::Lazy;
use regex::Regex;

pub fn filter_extensions_from_system_prompt(system: &str) -> String {
    let Some(extensions_start) = system.find("# Extensions") else {
        return system.to_string();
    };

    let Some(after_extensions) = system.get(extensions_start + 1..) else {
        return system.to_string();
    };

    if let Some(next_section_pos) = after_extensions.find("\n# ") {
        let Some(before) = system.get(..extensions_start) else {
            return system.to_string();
        };
        let Some(after) = system.get(extensions_start + next_section_pos + 1..) else {
            return system.to_string();
        };
        format!("{}{}", before.trim_end(), after)
    } else {
        system
            .get(..extensions_start)
            .map(|text| text.trim_end().to_string())
            .unwrap_or_else(|| system.to_string())
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct FilterOut {
    pub content: String,
    pub thinking: String,
}

pub struct ThinkFilter {
    buffer: String,
    inside_think: bool,
    think_depth: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThinkTag {
    Open,
    Close,
    SelfClosing,
}

enum BufferEvent {
    Tag {
        pos: usize,
        end: usize,
        kind: ThinkTag,
    },
    Partial(usize),
}

impl ThinkFilter {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            inside_think: false,
            think_depth: 0,
        }
    }

    pub fn push(&mut self, chunk: &str) -> FilterOut {
        self.buffer.push_str(chunk);
        self.process_buffer()
    }

    pub fn finish(mut self) -> FilterOut {
        let mut out = self.process_buffer();
        if !self.buffer.is_empty() {
            if self.inside_think {
                out.thinking.push_str(&self.buffer);
            } else {
                out.content.push_str(&self.buffer);
            }
            self.buffer.clear();
        }
        out
    }

    fn process_buffer(&mut self) -> FilterOut {
        let mut out = FilterOut::default();

        loop {
            match next_buffer_event(&self.buffer, self.inside_think) {
                Some(BufferEvent::Tag { pos, end, kind }) => {
                    if pos > 0 {
                        let prefix = self.buffer.get(..pos).unwrap_or_default().to_string();
                        if self.inside_think {
                            out.thinking.push_str(&prefix);
                        } else {
                            out.content.push_str(&prefix);
                        }
                    }

                    self.buffer.drain(..end);

                    match kind {
                        ThinkTag::Open => {
                            self.think_depth += 1;
                            self.inside_think = true;
                        }
                        ThinkTag::Close => {
                            self.think_depth = self.think_depth.saturating_sub(1);
                            self.inside_think = self.think_depth > 0;
                        }
                        ThinkTag::SelfClosing => {}
                    }
                }
                Some(BufferEvent::Partial(pos)) => {
                    if pos > 0 {
                        let prefix = self.buffer.get(..pos).unwrap_or_default().to_string();
                        if self.inside_think {
                            out.thinking.push_str(&prefix);
                        } else {
                            out.content.push_str(&prefix);
                        }
                        self.buffer.drain(..pos);
                    }
                    break;
                }
                None => {
                    if !self.buffer.is_empty() {
                        if self.inside_think {
                            out.thinking.push_str(&self.buffer);
                        } else {
                            out.content.push_str(&self.buffer);
                        }
                        self.buffer.clear();
                    }
                    break;
                }
            }
        }

        out
    }
}

impl Default for ThinkFilter {
    fn default() -> Self {
        Self::new()
    }
}

pub fn split_think_blocks(text: &str) -> (String, String) {
    let mut filter = ThinkFilter::new();
    let mut out = filter.push(text);
    let final_out = filter.finish();
    out.content.push_str(&final_out.content);
    out.thinking.push_str(&final_out.thinking);
    (out.content, out.thinking)
}

fn next_buffer_event(buffer: &str, inside_think: bool) -> Option<BufferEvent> {
    let mut search_from = 0;

    while let Some(rel_pos) = buffer.get(search_from..).and_then(|rest| rest.find('<')) {
        let pos = search_from + rel_pos;
        let suffix = buffer.get(pos..).unwrap_or_default();

        if let Some((kind, end)) = parse_think_tag(buffer, pos) {
            if inside_think || matches!(kind, ThinkTag::Open | ThinkTag::SelfClosing) {
                return Some(BufferEvent::Tag { pos, end, kind });
            }
        } else if !contains_unquoted_gt(suffix) && is_possible_partial_think_tag(suffix) {
            return Some(BufferEvent::Partial(pos));
        }

        search_from = pos + 1;
    }

    None
}

fn parse_think_tag(buffer: &str, start: usize) -> Option<(ThinkTag, usize)> {
    let bytes = buffer.as_bytes();
    if bytes.get(start) != Some(&b'<') {
        return None;
    }

    let mut idx = start + 1;
    let is_close = if bytes.get(idx) == Some(&b'/') {
        idx += 1;
        true
    } else {
        false
    };

    let name_start = idx;
    while bytes.get(idx).is_some_and(u8::is_ascii_alphabetic) {
        idx += 1;
    }

    if idx == name_start {
        return None;
    }

    let name = buffer.get(name_start..idx).unwrap_or_default();
    let is_think = name.eq_ignore_ascii_case("think") || name.eq_ignore_ascii_case("thinking");
    if !is_think {
        return None;
    }

    if is_close {
        while bytes.get(idx).is_some_and(u8::is_ascii_whitespace) {
            idx += 1;
        }
        if bytes.get(idx) == Some(&b'>') {
            return Some((ThinkTag::Close, idx + 1));
        }
        return None;
    }

    let valid_open_boundary = match bytes.get(idx) {
        Some(&b) => b == b'>' || b == b'/' || b.is_ascii_whitespace(),
        None => false,
    };
    if !valid_open_boundary {
        return None;
    }

    let mut quote: Option<u8> = None;
    let mut last_non_ws: Option<u8> = None;
    while let Some(&byte) = bytes.get(idx) {
        match quote {
            Some(quote_byte) => {
                if byte == quote_byte {
                    quote = None;
                }
            }
            None if matches!(byte, b'"' | b'\'') => {
                quote = Some(byte);
                last_non_ws = Some(byte);
            }
            None if byte == b'>' => {
                let kind = if last_non_ws == Some(b'/') {
                    ThinkTag::SelfClosing
                } else {
                    ThinkTag::Open
                };
                return Some((kind, idx + 1));
            }
            None if !byte.is_ascii_whitespace() => {
                last_non_ws = Some(byte);
            }
            None => {}
        }
        idx += 1;
    }

    None
}

fn is_possible_partial_think_tag(suffix: &str) -> bool {
    if contains_unquoted_gt(suffix) {
        return false;
    }

    static OPEN_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"(?s)^<[tT]([hH]([iI]([nN]([kK]([iI]([nN]([gG])?)?)?)?)?)?)(?:[ \t\r\n\f].*|/)?$",
        )
        .unwrap()
    });
    static CLOSE_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?s)^</[tT]([hH]([iI]([nN]([kK]([iI]([nN]([gG])?)?)?)?)?)?)(?:[ \t\r\n\f]*)?$")
            .unwrap()
    });

    OPEN_RE.is_match(suffix) || CLOSE_RE.is_match(suffix)
}

fn contains_unquoted_gt(text: &str) -> bool {
    let mut quote: Option<u8> = None;
    for &byte in text.as_bytes() {
        match quote {
            Some(quote_byte) => {
                if byte == quote_byte {
                    quote = None;
                }
            }
            None if matches!(byte, b'"' | b'\'') => quote = Some(byte),
            None if byte == b'>' => return true,
            None => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_extensions_section_from_system_prompt() {
        let system =
            "# Instructions\nBe helpful.\n\n# Extensions\nTool details.\n\n# Other\nKeep this.";
        assert_eq!(
            filter_extensions_from_system_prompt(system),
            "# Instructions\nBe helpful.\n# Other\nKeep this."
        );
    }

    #[test]
    fn filters_trailing_extensions_section_from_system_prompt() {
        let system = "# Instructions\nBe helpful.\n\n# Extensions\nTool details.";
        assert_eq!(
            filter_extensions_from_system_prompt(system),
            "# Instructions\nBe helpful."
        );
    }

    #[test]
    fn preserves_system_prompt_without_extensions_section() {
        let system = "# Instructions\nBe helpful.";
        assert_eq!(filter_extensions_from_system_prompt(system), system);
    }

    #[test]
    fn split_think_blocks_extracts_inline_reasoning() {
        assert_eq!(
            split_think_blocks("<think>x</think>y"),
            ("y".to_string(), "x".to_string())
        );
    }

    #[test]
    fn split_think_blocks_handles_thinking_variant() {
        assert_eq!(
            split_think_blocks("<thinking>a</thinking>b"),
            ("b".to_string(), "a".to_string())
        );
    }

    #[test]
    fn split_think_blocks_handles_quoted_gt_in_open_attributes() {
        assert_eq!(
            split_think_blocks(r#"<think data="a>b">Hidden</think>Visible"#),
            ("Visible".to_string(), "Hidden".to_string())
        );
    }

    #[test]
    fn think_filter_streaming_across_partial_tags() {
        let mut filter = ThinkFilter::new();
        let mut out = FilterOut::default();

        for chunk in ["<thi", "nk>x</thi", "nk>y"] {
            let partial = filter.push(chunk);
            out.content.push_str(&partial.content);
            out.thinking.push_str(&partial.thinking);
        }

        let final_out = filter.finish();
        out.content.push_str(&final_out.content);
        out.thinking.push_str(&final_out.thinking);

        assert_eq!(out.content, "y");
        assert_eq!(out.thinking, "x");
    }

    #[test]
    fn think_filter_preserves_non_think_tags() {
        let mut filter = ThinkFilter::new();
        let mut out = filter.push("<table>");
        let final_out = filter.finish();
        out.content.push_str(&final_out.content);
        out.thinking.push_str(&final_out.thinking);

        assert_eq!(out.content, "<table>");
        assert!(out.thinking.is_empty());
    }

    #[test]
    fn think_filter_treats_self_closing_as_noop() {
        for input in [
            "before <think/> after",
            "before <think /> after",
            "before <thinking/> after",
            "before <think data-source=\"x\"/> after",
        ] {
            let mut filter = ThinkFilter::new();
            let mut out = filter.push(input);
            let final_out = filter.finish();
            out.content.push_str(&final_out.content);
            out.thinking.push_str(&final_out.thinking);

            assert_eq!(
                out.content, "before  after",
                "content mismatch for {input:?}"
            );
            assert!(
                out.thinking.is_empty(),
                "unexpected thinking for {input:?}: {:?}",
                out.thinking
            );
        }
    }

    #[test]
    fn think_filter_streaming_across_quoted_attribute_boundary() {
        let mut filter = ThinkFilter::new();
        let mut out = filter.push(r#"<think data="a>b"#);
        assert!(out.content.is_empty());
        assert!(out.thinking.is_empty());

        let second = filter.push(r#""/>Visible"#);
        let final_out = filter.finish();
        out.content.push_str(&second.content);
        out.content.push_str(&final_out.content);
        out.thinking.push_str(&second.thinking);
        out.thinking.push_str(&final_out.thinking);

        assert_eq!(out.content, "Visible");
        assert!(out.thinking.is_empty());
    }
}
