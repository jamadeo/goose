use crate::errors::{GoogleErrorCode, ProviderError};
use crate::http_status::sanitize_url;
use reqwest::{Response, StatusCode};
use serde_json::Value;
use std::time::Duration;

fn format_server_error_message(status_code: StatusCode, payload: Option<&Value>) -> String {
    match payload {
        Some(Value::Null) | None => format!(
            "HTTP {}: No response body received from server",
            status_code.as_u16()
        ),
        Some(payload) => format!("HTTP {}: {}", status_code.as_u16(), payload),
    }
}

pub fn is_google_model(payload: &Value) -> bool {
    payload
        .get("model")
        .and_then(|model| model.as_str())
        .unwrap_or("")
        .to_lowercase()
        .contains("google")
}

fn get_google_final_status(status: StatusCode, payload: Option<&Value>) -> StatusCode {
    if status.is_success() {
        if let Some(payload) = payload {
            if let Some(error) = payload.get("error") {
                if let Some(code) = error.get("code").and_then(|code| code.as_u64()) {
                    if let Some(google_error) = GoogleErrorCode::from_code(code) {
                        return google_error.to_status_code();
                    }
                }
            }
        }
    }
    status
}

fn parse_google_retry_delay(payload: &Value) -> Option<Duration> {
    payload
        .get("error")
        .and_then(|error| error.get("details"))
        .and_then(|details| details.as_array())
        .and_then(|details| {
            details.iter().find_map(|detail| {
                if detail
                    .get("@type")
                    .and_then(|type_name| type_name.as_str())
                    .is_some_and(|type_name| type_name.ends_with("RetryInfo"))
                {
                    detail
                        .get("retryDelay")
                        .and_then(|delay| delay.as_str())
                        .and_then(|delay| delay.strip_suffix('s'))
                        .and_then(|seconds| seconds.parse::<u64>().ok())
                        .map(Duration::from_secs)
                } else {
                    None
                }
            })
        })
}

/// Handle response from Google Gemini API-compatible endpoints.
pub async fn handle_response_google_compat(response: Response) -> Result<Value, ProviderError> {
    let status = response.status();
    let url = sanitize_url(response.url().as_str());
    let payload: Option<Value> = response.json().await.ok();
    let final_status = get_google_final_status(status, payload.as_ref());

    match final_status {
        StatusCode::OK => payload
            .ok_or_else(|| ProviderError::RequestFailed("Response body is not valid JSON".to_string())),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ProviderError::Authentication(
            format!(
                "Authentication failed for {url}. Please ensure your API keys are valid and have the required permissions. Status: {}. Response: {:?}",
                final_status, payload
            ),
        )),
        StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND => {
            let mut error_msg = "Unknown error".to_string();
            if let Some(payload) = &payload {
                if let Some(error) = payload.get("error") {
                    error_msg = error
                        .get("message")
                        .and_then(|message| message.as_str())
                        .unwrap_or("Unknown error")
                        .to_string();
                    let error_status = error
                        .get("status")
                        .and_then(|status| status.as_str())
                        .unwrap_or("Unknown status");
                    if error_status == "INVALID_ARGUMENT"
                        && error_msg.to_lowercase().contains("exceeds")
                    {
                        return Err(ProviderError::ContextLengthExceeded(error_msg));
                    }
                }
            }
            tracing::debug!(
                "{}",
                format!(
                    "Provider request failed with status: {}. Payload: {:?}",
                    final_status, payload
                )
            );
            Err(ProviderError::RequestFailed(format!(
                "Request failed with status {final_status} at {url}. Message: {error_msg}"
            )))
        }
        StatusCode::TOO_MANY_REQUESTS => {
            let retry_delay = payload.as_ref().and_then(parse_google_retry_delay);
            Err(ProviderError::RateLimitExceeded {
                details: format!("{:?}", payload),
                retry_delay,
            })
        }
        _ if final_status.is_server_error() => Err(ProviderError::ServerError(format!(
            "Server error ({}) at {url}: {}",
            final_status,
            format_server_error_message(final_status, payload.as_ref())
        ))),
        _ => {
            tracing::debug!(
                "{}",
                format!(
                    "Provider request failed with status: {}. Payload: {:?}",
                    final_status, payload
                )
            );
            Err(ProviderError::RequestFailed(format!(
                "Request failed with status {final_status} at {url}"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_google_model_names() {
        for (payload, expected) in [
            (json!({ "model": "google_gemini" }), true),
            (json!({ "model": "microsoft_bing" }), false),
            (json!({ "model": "" }), false),
            (json!({}), false),
            (json!({ "model": "Google_XYZ" }), true),
            (json!({ "model": "google_abc" }), true),
        ] {
            assert_eq!(is_google_model(&payload), expected);
        }
    }

    #[test]
    fn google_final_status_preserves_success_without_error_payload() {
        let result = get_google_final_status(StatusCode::OK, Some(&json!({})));
        assert_eq!(result, StatusCode::OK);
    }

    #[test]
    fn google_final_status_uses_payload_error_code_for_success_statuses() {
        for (error_code, status, expected_status) in [
            (200, None, StatusCode::OK),
            (429, Some(StatusCode::OK), StatusCode::TOO_MANY_REQUESTS),
            (400, Some(StatusCode::OK), StatusCode::BAD_REQUEST),
            (401, Some(StatusCode::OK), StatusCode::UNAUTHORIZED),
            (403, Some(StatusCode::OK), StatusCode::FORBIDDEN),
            (404, Some(StatusCode::OK), StatusCode::NOT_FOUND),
            (500, Some(StatusCode::OK), StatusCode::INTERNAL_SERVER_ERROR),
            (503, Some(StatusCode::OK), StatusCode::SERVICE_UNAVAILABLE),
            (999, Some(StatusCode::OK), StatusCode::INTERNAL_SERVER_ERROR),
            (500, Some(StatusCode::BAD_REQUEST), StatusCode::BAD_REQUEST),
            (
                404,
                Some(StatusCode::INTERNAL_SERVER_ERROR),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ] {
            let payload = if status.is_some() {
                json!({
                    "error": {
                        "code": error_code,
                        "message": "Error message"
                    }
                })
            } else {
                json!({})
            };

            let result = get_google_final_status(status.unwrap_or(StatusCode::OK), Some(&payload));
            assert_eq!(result, expected_status);
        }
    }

    #[test]
    fn parses_google_retry_delay() {
        let payload = json!({
            "error": {
                "details": [
                    {
                        "@type": "type.googleapis.com/google.rpc.RetryInfo",
                        "retryDelay": "42s"
                    }
                ]
            }
        });
        assert_eq!(
            parse_google_retry_delay(&payload),
            Some(Duration::from_secs(42))
        );
    }
}
