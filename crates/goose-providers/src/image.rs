use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageFormat {
    OpenAi,
    Anthropic,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_format_round_trips_through_json() {
        let encoded = serde_json::to_string(&ImageFormat::OpenAi).unwrap();
        assert_eq!(encoded, "\"OpenAi\"");
        let decoded: ImageFormat = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, ImageFormat::OpenAi);
    }
}
