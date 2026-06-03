use crate::errors::ProviderError;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Read;
use std::path::Path;

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

#[derive(Debug)]
pub struct EncodedImage {
    pub mime_type: String,
    pub data: String,
}

fn is_image_file(path: &Path) -> bool {
    if let Ok(mut file) = std::fs::File::open(path) {
        let mut buffer = [0u8; 8];
        if file.read(&mut buffer).is_ok() {
            return matches!(
                &buffer[0..4],
                [0x89, 0x50, 0x4E, 0x47] | [0xFF, 0xD8, 0xFF, _] | [0x47, 0x49, 0x46, 0x38]
            );
        }
    }
    false
}

pub fn detect_image_path(text: &str) -> Option<&str> {
    let extensions = [".png", ".jpg", ".jpeg"];

    for word in text.split_whitespace() {
        if extensions
            .iter()
            .any(|ext| word.to_lowercase().ends_with(ext))
        {
            let path = Path::new(word);
            if path.is_absolute() && path.is_file() && is_image_file(path) {
                return Some(word);
            }
        }
    }
    None
}

pub fn load_image_file(path: &str) -> Result<EncodedImage, ProviderError> {
    let path = Path::new(path);

    if !is_image_file(path) {
        return Err(ProviderError::RequestFailed(
            "File is not a valid image".to_string(),
        ));
    }

    let bytes = std::fs::read(path).map_err(|error| {
        ProviderError::RequestFailed(format!("Failed to read image file: {error}"))
    })?;

    let mime_type = match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) => match extension.to_lowercase().as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            _ => {
                return Err(ProviderError::RequestFailed(
                    "Unsupported image format".to_string(),
                ))
            }
        },
        None => {
            return Err(ProviderError::RequestFailed(
                "Unknown image format".to_string(),
            ))
        }
    };

    Ok(EncodedImage {
        mime_type: mime_type.to_string(),
        data: base64::prelude::BASE64_STANDARD.encode(&bytes),
    })
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

    fn write_file(path: &std::path::Path, data: &[u8]) {
        std::fs::write(path, data).unwrap();
    }

    #[test]
    fn detects_absolute_image_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let png_path = temp_dir.path().join("test.png");
        let png_data = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
        ];
        write_file(&png_path, &png_data);
        let png_path_str = png_path.to_str().unwrap();

        let fake_png_path = temp_dir.path().join("fake.png");
        write_file(&fake_png_path, b"not a real png");

        let text = format!("Here is an image {png_path_str}");
        assert_eq!(detect_image_path(&text), Some(png_path_str));

        let text = format!("Here is a fake image {}", fake_png_path.to_str().unwrap());
        assert_eq!(detect_image_path(&text), None);

        assert_eq!(
            detect_image_path("Here is a fake.png that doesn't exist"),
            None
        );
        assert_eq!(detect_image_path("Here is a file.txt"), None);
        assert_eq!(detect_image_path("Here is a relative/path/image.png"), None);
    }

    #[test]
    fn loads_supported_image_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let png_path = temp_dir.path().join("test.png");
        let png_data = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
        ];
        write_file(&png_path, &png_data);

        let image = load_image_file(png_path.to_str().unwrap()).unwrap();
        assert_eq!(image.mime_type, "image/png");
        assert!(!image.data.is_empty());
    }

    #[test]
    fn rejects_invalid_or_unsupported_image_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let fake_png_path = temp_dir.path().join("fake.png");
        write_file(&fake_png_path, b"not a real png");
        let result = load_image_file(fake_png_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not a valid image"));

        let result = load_image_file("nonexistent.png");
        assert!(result.is_err());

        let gif_path = temp_dir.path().join("test.gif");
        let gif_data = [0x47, 0x49, 0x46, 0x38, 0x39, 0x61];
        write_file(&gif_path, &gif_data);
        let result = load_image_file(gif_path.to_str().unwrap());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported image format"));
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
