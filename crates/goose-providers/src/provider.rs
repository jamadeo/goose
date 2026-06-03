use crate::errors::ProviderError;
use crate::models;
use crate::retry::RetryConfig;
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use goose_types::{Message, MessageContent, ModelConfig, ModelInfo, ProviderUsage, Usage};
use rmcp::model::Tool;
use std::pin::Pin;

pub type MessageStream = Pin<
    Box<dyn Stream<Item = Result<(Option<Message>, Option<ProviderUsage>), ProviderError>> + Send>,
>;

pub fn stream_from_single_message(message: Message, usage: ProviderUsage) -> MessageStream {
    let stream = futures::stream::once(async move { Ok((Some(message), Some(usage))) });
    Box::pin(stream)
}

pub async fn collect_stream(
    mut stream: MessageStream,
) -> Result<(Message, ProviderUsage), ProviderError> {
    let mut final_message: Option<Message> = None;
    let mut final_usage: Option<ProviderUsage> = None;

    while let Some(result) = stream.next().await {
        let (msg_opt, usage_opt) = result?;

        if let Some(msg) = msg_opt {
            final_message = Some(match final_message {
                Some(mut prev) => {
                    for new_content in msg.content {
                        match (&mut prev.content.last_mut(), &new_content) {
                            (
                                Some(MessageContent::Text(last_text)),
                                MessageContent::Text(new_text),
                            ) => {
                                last_text.text.push_str(&new_text.text);
                            }
                            _ => {
                                prev.content.push(new_content);
                            }
                        }
                    }
                    prev
                }
                None => msg,
            });
        }

        if let Some(usage) = usage_opt {
            final_usage = Some(usage);
        }
    }

    match final_message {
        Some(msg) => {
            let usage = final_usage
                .unwrap_or_else(|| ProviderUsage::new("unknown".to_string(), Usage::default()));
            Ok((msg, usage))
        }
        None => Err(ProviderError::ExecutionError(
            "Stream yielded no message".to_string(),
        )),
    }
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn get_name(&self) -> &str;

    async fn stream(
        &self,
        model_config: &ModelConfig,
        session_id: &str,
        system: &str,
        messages: &[Message],
        tools: &[Tool],
    ) -> Result<MessageStream, ProviderError>;

    fn get_model_config(&self) -> ModelConfig;

    fn retry_config(&self) -> RetryConfig {
        RetryConfig::default()
    }

    async fn fetch_supported_models(&self) -> Result<Vec<String>, ProviderError> {
        Ok(vec![])
    }

    async fn fetch_supported_model_info(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(self
            .fetch_supported_models()
            .await?
            .iter()
            .map(|model_name| models::model_info_for_provider_model(self.get_name(), model_name))
            .collect())
    }

    async fn fetch_model_info(&self, model_name: &str) -> Result<ModelInfo, ProviderError> {
        Ok(models::model_info_for_provider_model(
            self.get_name(),
            model_name,
        ))
    }

    fn skip_canonical_filtering(&self) -> bool {
        false
    }

    async fn fetch_recommended_models(&self) -> Result<Vec<String>, ProviderError> {
        models::recommended_models(
            self.get_name(),
            self.fetch_supported_models().await?,
            self.get_model_config().toolshim,
            self.skip_canonical_filtering(),
        )
    }

    async fn fetch_recommended_model_info(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(self
            .fetch_recommended_models()
            .await?
            .iter()
            .map(|model_name| models::model_info_for_provider_model(self.get_name(), model_name))
            .collect())
    }

    async fn map_to_canonical_model(
        &self,
        provider_model: &str,
    ) -> Result<Option<String>, ProviderError> {
        models::canonical_model_id(self.get_name(), provider_model)
    }

    fn supports_embeddings(&self) -> bool {
        false
    }

    fn manages_own_context(&self) -> bool {
        false
    }

    async fn supports_cache_control(&self) -> bool {
        false
    }

    async fn create_embeddings(
        &self,
        _session_id: &str,
        _texts: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        Err(ProviderError::ExecutionError(
            "This provider does not support embeddings".to_string(),
        ))
    }

    async fn configure_oauth(&self) -> Result<(), ProviderError> {
        Err(ProviderError::ExecutionError(
            "OAuth configuration not supported by this provider".to_string(),
        ))
    }

    async fn refresh_credentials(&self) -> Result<(), ProviderError> {
        Err(ProviderError::NotImplemented(
            "credential refresh not supported by this provider".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::Stream;
    use goose_types::{MessageContent, Usage};
    use rmcp::model::{CallToolRequestParams, Role};
    use test_case::test_case;

    fn content_from_str(s: String) -> MessageContent {
        if let Some(img_data) = s.strip_prefix("*img:") {
            MessageContent::image(format!("http://example.com/{img_data}"), "image/png")
        } else if let Some(tool_name) = s.strip_prefix("*tool:") {
            let tool_call = Ok(CallToolRequestParams::new(tool_name.to_string())
                .with_arguments(serde_json::Map::new()));
            MessageContent::tool_request(format!("tool_{tool_name}"), tool_call)
        } else {
            MessageContent::text(s)
        }
    }

    fn create_test_stream(
        items: Vec<String>,
    ) -> impl Stream<Item = Result<(Option<Message>, Option<ProviderUsage>), ProviderError>> {
        use futures::stream;
        stream::iter(items.into_iter().map(|item| {
            let content = content_from_str(item);
            let message = Message::new(Role::Assistant, 0, vec![content]);
            Ok((Some(message), None))
        }))
    }

    fn content_to_strings(msg: &Message) -> Vec<String> {
        msg.content
            .iter()
            .map(|c| match c {
                MessageContent::Text(t) => t.text.clone(),
                MessageContent::Image(_) => "*img".to_string(),
                MessageContent::ToolRequest(tr) => match &tr.tool_call {
                    Ok(call) => format!("*tool:{}", call.name),
                    Err(_) => "*tool:error".to_string(),
                },
                _ => "*other".to_string(),
            })
            .collect()
    }

    #[test_case(
        vec!["Hello", " ", "world"],
        vec!["Hello world"]
        ; "consecutive text coalesces"
    )]
    #[test_case(
        vec!["Hello", "*img:pic1", "world"],
        vec!["Hello", "*img", "world"]
        ; "non-text breaks coalescing"
    )]
    #[test_case(
        vec!["A", "B", "*img:pic1", "C", "D", "*tool:read", "E", "F"],
        vec!["AB", "*img", "CD", "*tool:read", "EF"]
        ; "multiple text groups"
    )]
    #[tokio::test]
    async fn collect_stream_coalesces_text_content(input_items: Vec<&str>, expected: Vec<&str>) {
        let items: Vec<String> = input_items.into_iter().map(|s| s.to_string()).collect();
        let stream = create_test_stream(items);
        let (msg, _) = collect_stream(Box::pin(stream)).await.unwrap();
        assert_eq!(content_to_strings(&msg), expected);
    }

    #[tokio::test]
    async fn collect_stream_defaults_usage() {
        let stream = create_test_stream(vec!["Hello".to_string()]);
        let (msg, usage) = collect_stream(Box::pin(stream)).await.unwrap();
        assert_eq!(content_to_strings(&msg), vec!["Hello"]);
        assert_eq!(usage.model, "unknown");
    }

    #[tokio::test]
    async fn stream_from_single_message_round_trips_message_and_usage() {
        let message = Message::assistant().with_text("done");
        let usage = ProviderUsage::new("test-model".to_string(), Usage::default());

        let (message, usage) = collect_stream(stream_from_single_message(message, usage))
            .await
            .unwrap();

        assert_eq!(message.as_concat_text(), "done");
        assert_eq!(usage.model, "test-model");
    }
}
