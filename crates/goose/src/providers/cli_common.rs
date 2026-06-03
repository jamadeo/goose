use super::base::ProviderUsage;
use super::errors::ProviderError;
use crate::conversation::message::{Message, MessageContent};
use rmcp::model::Role;

pub(crate) use goose_providers::cli::{
    error_from_event, extract_usage_tokens, is_session_description_request,
    simple_session_description_from_text, SESSION_NAME_BEGIN_MARKER, SESSION_NAME_END_MARKER,
    SESSION_NAME_SUFFIX,
};

pub(crate) fn generate_simple_session_description(
    model_name: &str,
    messages: &[Message],
) -> Result<(Message, ProviderUsage), ProviderError> {
    let description = messages
        .iter()
        .find(|message| message.role == Role::User)
        .and_then(|message| {
            message.content.iter().find_map(|content| match content {
                MessageContent::Text(text_content) => Some(&text_content.text),
                _ => None,
            })
        })
        .map(|text| simple_session_description_from_text(text))
        .unwrap_or_else(|| "Simple task".to_string());

    tracing::debug!(
        description = %description,
        "Generated simple session description, skipped subprocess"
    );

    let message = Message::new(
        Role::Assistant,
        chrono::Utc::now().timestamp(),
        vec![MessageContent::text(description)],
    );

    Ok((
        message,
        ProviderUsage::new(model_name.to_string(), Default::default()),
    ))
}
