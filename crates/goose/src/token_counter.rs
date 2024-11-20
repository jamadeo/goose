use include_dir::{include_dir, Dir};
use std::collections::HashMap;
use tokenizers::tokenizer::Tokenizer;

// Embed the tokenizer files directory
static TOKENIZER_FILES: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../tokenizer_files");

pub struct TokenCounter {
    tokenizers: HashMap<String, Tokenizer>,
}

const GPT_4O_TOKENIZER_KEY: &str = "Xenova--gpt-4o";
const CLAUDE_TOKENIZER_KEY: &str = "Xenova--claude-tokenizer";
const QWEN_TOKENIZER_KEY: &str = "Qwen--Qwen2.5-Coder-32B-Instruct";

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenCounter {
    fn load_tokenizer(&mut self, tokenizer_key: &str) {
        // Load from embedded tokenizer files. The tokenizer_key must match the directory name.
        let tokenizer_path = format!("{}/tokenizer.json", tokenizer_key);
        let file_content = TOKENIZER_FILES
            .get_file(&tokenizer_path)
            .map(|f| f.contents())
            .ok_or_else(|| format!("Embedded tokenizer file not found: {}", tokenizer_path))
            .unwrap();

        let tokenizer = Tokenizer::from_bytes(file_content);

        match tokenizer {
            Ok(tokenizer) => {
                self.tokenizers.insert(tokenizer_key.to_string(), tokenizer);
            }
            Err(e) => {
                eprintln!("Failed to load tokenizer {}: {}", tokenizer_key, e);
            }
        }
    }

    pub fn new() -> Self {
        let mut counter = TokenCounter {
            tokenizers: HashMap::new(),
        };
        // Add default tokenizers
        for tokenizer_key in [GPT_4O_TOKENIZER_KEY, CLAUDE_TOKENIZER_KEY] {
            counter.load_tokenizer(tokenizer_key);
        }
        counter
    }

    pub fn add_tokenizer(&mut self, tokenizer_key: &str) {
        self.load_tokenizer(tokenizer_key);
    }

    fn model_to_tokenizer_key(model_name: Option<&str>) -> &str {
        let model_name = model_name.unwrap_or("gpt-4o").to_lowercase();
        if model_name.contains("claude") {
            CLAUDE_TOKENIZER_KEY
        } else if model_name.contains("qwen") {
            QWEN_TOKENIZER_KEY
        } else {
            // default
            GPT_4O_TOKENIZER_KEY
        }
    }

    pub fn count_tokens(&self, text: &str, model_name: Option<&str>) -> usize {
        let tokenizer_key = Self::model_to_tokenizer_key(model_name);
        dbg!(&model_name, &tokenizer_key);
        let tokenizer = self
            .tokenizers
            .get(tokenizer_key)
            .expect("Tokenizer not found");
        let encoding = tokenizer.encode(text, false).unwrap();
        encoding.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_tokenizer_and_count_tokens() {
        let mut counter = TokenCounter::new();
        counter.add_tokenizer(QWEN_TOKENIZER_KEY);
        let text = "Hey there!";
        let count = counter.count_tokens(text, Some("qwen2.5-ollama"));
        println!("Token count for '{}': {:?}", text, count);

        assert_eq!(count, 3);
    }

    // Update the default tokenizer test similarly
    #[test]
    fn test_specific_claude_tokenizer() {
        let counter = TokenCounter::new();
        let text = "Hello, how are you?";
        let count = counter.count_tokens(text, Some("claude-3-5-sonnet-2"));

        println!("Token count for '{}': {:?}", text, count);
        assert_eq!(count, 6);
    }

    #[test]
    fn test_default_gpt_4o_tokenizer() {
        let counter = TokenCounter::new();
        let count = counter.count_tokens("Hey there!", None);
        assert_eq!(count, 3);
    }
}
