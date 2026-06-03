use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageFormat {
    OpenAi,
    Anthropic,
}

pub fn convert_image(data: &str, mime_type: &str, image_format: &ImageFormat) -> Value {
    match image_format {
        ImageFormat::OpenAi => json!({
            "type": "image_url",
            "image_url": {
                "url": format!("data:{};base64,{}", mime_type, data)
            }
        }),
        ImageFormat::Anthropic => json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": mime_type,
                "data": data,
            }
        }),
    }
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

    #[test]
    fn converts_image_to_openai_payload() {
        assert_eq!(
            convert_image("abc123", "image/png", &ImageFormat::OpenAi),
            serde_json::json!({
                "type": "image_url",
                "image_url": {
                    "url": "data:image/png;base64,abc123"
                }
            })
        );
    }

    #[test]
    fn converts_image_to_anthropic_payload() {
        assert_eq!(
            convert_image("abc123", "image/png", &ImageFormat::Anthropic),
            serde_json::json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": "image/png",
                    "data": "abc123",
                }
            })
        );
    }
}
