use crate::errors::ProviderError;
use crate::json::safely_parse_json;
use crate::text::ThinkFilter;
use anyhow::anyhow;
use async_stream::try_stream;
use futures::Stream;
use goose_types::{Message, MessageContent, MessageProviderMetadata, ProviderUsage, Usage};
use rmcp::model::{object, CallToolRequestParams, ErrorCode, ErrorData, Role};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::borrow::Cow;
use std::collections::HashMap;

pub const THOUGHT_SIGNATURE_KEY: &str = "thoughtSignature";

pub type ToolCallData = HashMap<
    i32,
    (
        String,
        String,
        String,
        Option<serde_json::Map<String, Value>>,
    ),
>;

#[derive(Debug, Clone, Copy, Default)]
pub struct OpenAiFormatOptions {
    pub preserve_thinking_context: bool,
}

fn deserialize_null_default_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct DeltaToolCallFunction {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default_string")]
    pub arguments: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeltaToolCall {
    pub id: Option<String>,
    pub function: DeltaToolCallFunction,
    pub index: Option<i32>,
    pub r#type: Option<String>,
    #[serde(flatten)]
    pub extra: Option<serde_json::Map<String, Value>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum DeltaContent {
    String(String),
    Array(Vec<ChatContentPart>),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChatContentPart {
    pub r#type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(rename = "thoughtSignature")]
    pub thought_signature: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Delta {
    #[serde(default)]
    pub content: Option<DeltaContent>,
    pub role: Option<String>,
    pub tool_calls: Option<Vec<DeltaToolCall>>,
    pub reasoning_details: Option<Vec<Value>>,
    pub reasoning: Option<String>,
    pub reasoning_content: Option<String>,
}

impl Delta {
    pub fn reasoning_text(&self) -> Option<&str> {
        self.reasoning_content
            .as_deref()
            .filter(|s| !s.is_empty())
            .or_else(|| self.reasoning.as_deref().filter(|s| !s.is_empty()))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StreamingChoice {
    #[serde(default)]
    pub delta: Delta,
    pub index: Option<i32>,
    pub finish_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StreamingChunk {
    pub choices: Vec<StreamingChoice>,
    pub created: Option<i64>,
    pub id: Option<String>,
    pub usage: Option<Value>,
    pub model: Option<String>,
}

pub fn merge_reasoning_text(prefix: &str, suffix: &str) -> String {
    if prefix.is_empty() {
        return suffix.to_string();
    }
    if suffix.is_empty() {
        return prefix.to_string();
    }
    if suffix.starts_with(prefix) {
        return suffix.to_string();
    }
    if prefix.ends_with(suffix) {
        return prefix.to_string();
    }

    format!("{prefix}{suffix}")
}

pub fn extract_content_and_signature(
    delta_content: Option<&DeltaContent>,
) -> (Option<String>, Option<String>) {
    match delta_content {
        Some(DeltaContent::String(s)) => (Some(s.clone()), None),
        Some(DeltaContent::Array(parts)) => {
            let text_parts: Vec<_> = parts.iter().filter(|p| p.r#type == "text").collect();

            let text = text_parts
                .iter()
                .filter_map(|p| p.text.as_deref())
                .collect::<String>();

            let signature = text_parts
                .iter()
                .find_map(|p| p.thought_signature.as_ref())
                .cloned();

            let text = if text.is_empty() { None } else { Some(text) };

            (text, signature)
        }
        None => (None, None),
    }
}

pub fn get_usage(usage: &Value) -> Usage {
    let usage = usage
        .get("usage")
        .filter(|nested| nested.is_object())
        .unwrap_or(usage);

    // Try standard OpenAI fields first, then fall back to Ollama-native fields
    // (prompt_eval_count / eval_count) for compatibility with older Ollama builds
    // that don't translate to OpenAI field names.
    // Parse the value before falling back so that present-but-null keys
    // (e.g. "completion_tokens": null) don't block the fallback.
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| usage.get("prompt_eval_count").and_then(|v| v.as_i64()))
        .map(|v| v as i32);

    let output_tokens = usage
        .get("completion_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| usage.get("eval_count").and_then(|v| v.as_i64()))
        .map(|v| v as i32);

    let cache_read_input_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);

    let cache_write_input_tokens = usage
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);

    let total_tokens = usage
        .get("total_tokens")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32)
        .or_else(|| match (input_tokens, output_tokens) {
            (Some(input), Some(output)) => Some(input.saturating_add(output)),
            _ => None,
        });

    Usage::new(input_tokens, output_tokens, total_tokens)
        .with_cache_tokens(cache_read_input_tokens, cache_write_input_tokens)
}

pub fn extract_usage_with_output_tokens(
    chunk: &StreamingChunk,
    fallback_model: Option<&str>,
) -> Option<ProviderUsage> {
    chunk
        .usage
        .as_ref()
        .and_then(|u| {
            chunk
                .model
                .as_deref()
                .or(fallback_model)
                .map(|model| ProviderUsage {
                    usage: get_usage(u),
                    model: model.to_string(),
                })
        })
        .filter(|u| u.usage.output_tokens.is_some())
}

pub fn strip_data_prefix(line: &str) -> Option<&str> {
    // SSE spec allows both "data: value" and "data:value" (space after colon is optional)
    line.strip_prefix("data: ")
        .or_else(|| line.strip_prefix("data:"))
        .map(|s| s.trim())
}

pub fn parse_streaming_chunk(line: &str) -> Result<StreamingChunk, ProviderError> {
    let value: Value = serde_json::from_str(line).map_err(|e| {
        ProviderError::RequestFailed(format!("Failed to parse streaming chunk: {e}: {line:?}"))
    })?;

    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown server error");
        return Err(ProviderError::ServerError(message.to_string()));
    }

    if value.get("object").and_then(|o| o.as_str()) == Some("error") {
        let message = value
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown server error");
        return Err(ProviderError::ServerError(message.to_string()));
    }

    serde_json::from_value(value).map_err(|e| {
        ProviderError::RequestFailed(format!("Failed to parse streaming chunk: {e}: {line:?}"))
    })
}

pub fn response_to_streaming_message<S>(
    mut stream: S,
) -> impl Stream<Item = anyhow::Result<(Option<Message>, Option<ProviderUsage>)>> + 'static
where
    S: Stream<Item = anyhow::Result<String>> + Unpin + Send + 'static,
{
    try_stream! {
        use futures::StreamExt;

        let mut accumulated_reasoning: Vec<Value> = Vec::new();
        let mut accumulated_reasoning_content = String::new();
        let mut think_filter = ThinkFilter::new();
        let mut saw_structured_reasoning = false;
        let mut yielded_reasoning_content_len = 0usize;
        let mut last_signature: Option<String> = None;
        // Buffer inline <think>...</think> content until we know whether structured
        // reasoning will arrive. Emitting it immediately and then receiving
        // reasoning_content in a later chunk would produce duplicated reasoning.
        let mut pending_inline_thinking = String::new();
        let mut last_seen_model: Option<String> = None;

        'outer: while let Some(response) = stream.next().await {
            let response_str = response?;
            let line = strip_data_prefix(&response_str);

            if line.is_some_and(|l| l == "[DONE]") {
                break 'outer;
            }

            if line.is_none() || line.is_some_and(|l| l.is_empty()) {
                continue
            }

            let chunk: StreamingChunk = parse_streaming_chunk(
                line.ok_or_else(|| anyhow!("unexpected stream format"))?
            )?;
            if let Some(model) = &chunk.model {
                last_seen_model = Some(model.clone());
            }

            if !chunk.choices.is_empty() {
                if let Some(details) = &chunk.choices[0].delta.reasoning_details {
                    accumulated_reasoning.extend(details.iter().cloned());
                }
                if let Some(rc) = chunk.choices[0].delta.reasoning_text() {
                    accumulated_reasoning_content.push_str(rc);
                    if !rc.is_empty() {
                        saw_structured_reasoning = true;
                        pending_inline_thinking.clear();
                    }
                }
            }

            let mut usage = extract_usage_with_output_tokens(&chunk, last_seen_model.as_deref());

            if chunk.choices.is_empty() {
                yield (None, usage)
            } else if chunk.choices[0].delta.tool_calls.as_ref().is_some_and(|tc| !tc.is_empty()) {
                let mut tool_call_data: ToolCallData = HashMap::new();

                if let Some(tool_calls) = &chunk.choices[0].delta.tool_calls {
                    for tool_call in tool_calls {
                        if let (Some(index), Some(id), Some(name)) = (tool_call.index, &tool_call.id, &tool_call.function.name) {
                            tool_call_data.insert(index, (id.clone(), name.clone(), tool_call.function.arguments.clone(), tool_call.extra.clone()));
                        }
                    }
                }

                let is_complete = chunk.choices[0].finish_reason == Some("tool_calls".to_string());

                if !is_complete {
                    let mut done = false;
                    while !done {
                        if let Some(response_chunk) = stream.next().await {
                            let response_str = response_chunk?;
                            if let Some(line) = strip_data_prefix(&response_str) {
                                if line == "[DONE]" {
                                    break 'outer;
                                }

                                let tool_chunk: StreamingChunk = parse_streaming_chunk(line)?;
                                if let Some(model) = &tool_chunk.model {
                                    last_seen_model = Some(model.clone());
                                }

                                if let Some(chunk_usage) = extract_usage_with_output_tokens(&tool_chunk, last_seen_model.as_deref()) {
                                    usage = Some(chunk_usage);
                                }

                                if !tool_chunk.choices.is_empty() {
                                    if let Some(details) = &tool_chunk.choices[0].delta.reasoning_details {
                                        accumulated_reasoning.extend(details.iter().cloned());
                                    }
                                    if let Some(rc) = tool_chunk.choices[0].delta.reasoning_text() {
                                        accumulated_reasoning_content.push_str(rc);
                                        if !rc.is_empty() {
                                            saw_structured_reasoning = true;
                                            pending_inline_thinking.clear();
                                        }
                                    }
                                    if let Some(delta_tool_calls) = &tool_chunk.choices[0].delta.tool_calls {
                                        for delta_call in delta_tool_calls {
                                            if let Some(index) = delta_call.index {
                                                if let Some((_, _, ref mut args, ref mut extra)) = tool_call_data.get_mut(&index) {
                                                    args.push_str(&delta_call.function.arguments);
                                                    if extra.is_none() && delta_call.extra.is_some() {
                                                        *extra = delta_call.extra.clone();
                                                    } else if let (Some(existing), Some(new_extra)) = (extra.as_mut(), &delta_call.extra) {
                                                        for (key, value) in new_extra {
                                                            existing.entry(key.clone()).or_insert(value.clone());
                                                        }
                                                    }
                                                } else if let (Some(id), Some(name)) = (&delta_call.id, &delta_call.function.name) {
                                                    tool_call_data.insert(index, (id.clone(), name.clone(), delta_call.function.arguments.clone(), delta_call.extra.clone()));
                                                }
                                            }
                                        }
                                    }
                                    if tool_chunk.choices[0].finish_reason.is_some() {
                                        done = true;
                                    }
                                } else {
                                    done = true;
                                }
                            }
                        } else {
                            break;
                        }
                    }
                }

                let _metadata: Option<MessageProviderMetadata> = if !accumulated_reasoning.is_empty() {
                    let mut map = MessageProviderMetadata::new();
                    map.insert("reasoning_details".to_string(), json!(accumulated_reasoning));
                    Some(map)
                } else {
                    None
                };

                let filtered = think_filter.push("");
                let mut flush_thinking = String::new();
                if !saw_structured_reasoning {
                    flush_thinking.push_str(&pending_inline_thinking);
                    flush_thinking.push_str(&filtered.thinking);
                }
                pending_inline_thinking.clear();
                if !filtered.content.is_empty() || !flush_thinking.is_empty() {
                    let mut filtered_contents = Vec::new();
                    if !filtered.content.is_empty() {
                        filtered_contents.push(MessageContent::text(filtered.content));
                    }
                    if !flush_thinking.is_empty() {
                        filtered_contents.push(MessageContent::thinking(flush_thinking, ""));
                    }

                    if !filtered_contents.is_empty() {
                        let mut msg = Message::new(
                            Role::Assistant,
                            chrono::Utc::now().timestamp(),
                            filtered_contents,
                        );

                        if let Some(id) = chunk.id.clone() {
                            msg = msg.with_id(id);
                        }

                        yield (Some(msg), None);
                    }
                }

                let mut contents = Vec::new();
                if yielded_reasoning_content_len < accumulated_reasoning_content.len() {
                    if let Some(unyielded_reasoning) =
                        accumulated_reasoning_content.get(yielded_reasoning_content_len..)
                    {
                        if !unyielded_reasoning.is_empty() {
                            contents.push(MessageContent::thinking(unyielded_reasoning, ""));
                        }
                    }
                }
                accumulated_reasoning_content.clear();
                yielded_reasoning_content_len = 0;
                let mut sorted_indices: Vec<_> = tool_call_data.keys().cloned().collect();
                sorted_indices.sort();

                for index in sorted_indices {
                    if let Some((id, function_name, arguments, extra_fields)) = tool_call_data.get(&index) {
                        let parsed = if arguments.is_empty() {
                            Ok(json!({}))
                        } else {
                            safely_parse_json(arguments)
                        };

                        let metadata = if let Some(sig) = &last_signature {
                            let mut combined = extra_fields.clone().unwrap_or_default();
                            combined.insert(
                                THOUGHT_SIGNATURE_KEY.to_string(),
                                json!(sig)
                            );
                            Some(combined)
                        } else {
                            extra_fields.as_ref().filter(|m| !m.is_empty()).cloned()
                        };

                        let content = match parsed {
                            Ok(params) => {
                                MessageContent::tool_request_with_metadata(
                                    id.clone(),
                                    Ok(CallToolRequestParams::new(function_name.clone()).with_arguments(object(params))),
                                    metadata.as_ref(),
                                )
                            },
                            Err(e) => {
                                let error = ErrorData {
                                    code: ErrorCode::INVALID_PARAMS,
                                    message: Cow::from(format!(
                                        "Could not interpret tool use parameters for id {}: {}",
                                        id, e
                                    )),
                                    data: None,
                                };
                                MessageContent::tool_request_with_metadata(id.clone(), Err(error), metadata.as_ref())
                            }
                        };
                        contents.push(content);
                    }
                }

                let mut msg = Message::new(
                    Role::Assistant,
                    chrono::Utc::now().timestamp(),
                    contents,
                );

                // Add ID if present
                if let Some(id) = chunk.id {
                    msg = msg.with_id(id);
                }

                yield (
                    Some(msg),
                    usage,
                )
            } else if chunk.choices[0].delta.content.is_some() || chunk.choices[0].delta.reasoning_text().is_some() {
                let mut content = Vec::new();

                if let Some(reasoning) = chunk.choices[0].delta.reasoning_text() {
                    let signature = last_signature.as_deref().unwrap_or("");
                    content.push(MessageContent::thinking(reasoning, signature));
                    yielded_reasoning_content_len = accumulated_reasoning_content.len();
                }

                let (text_content, thought_signature) = extract_content_and_signature(chunk.choices[0].delta.content.as_ref());

                if let Some(sig) = thought_signature {
                    last_signature = Some(sig);
                }

                if let Some(text) = text_content {
                    let filtered = think_filter.push(&text);

                    if !saw_structured_reasoning && !filtered.thinking.is_empty() {
                        pending_inline_thinking.push_str(&filtered.thinking);
                    }

                    if !filtered.content.is_empty() {
                        content.push(MessageContent::text(filtered.content));
                    }
                }

                if !content.is_empty() {
                    let mut msg = Message::new(
                        Role::Assistant,
                        chrono::Utc::now().timestamp(),
                        content,
                    );

                    if let Some(id) = chunk.id {
                        msg = msg.with_id(id);
                    }

                    yield (
                        Some(msg),
                        if chunk.choices[0].finish_reason.is_some() {
                            usage
                        } else {
                            None
                        },
                    )
                } else if usage.is_some() {
                    yield (None, usage)
                }
            } else if usage.is_some() {
                yield (None, usage)
            }
        }

        let filtered = think_filter.finish();
        let mut trailing_thinking = String::new();
        if !saw_structured_reasoning {
            trailing_thinking.push_str(&pending_inline_thinking);
            trailing_thinking.push_str(&filtered.thinking);
        }
        pending_inline_thinking.clear();

        if !filtered.content.is_empty() || !trailing_thinking.is_empty() {
            let mut content = Vec::new();

            if !filtered.content.is_empty() {
                content.push(MessageContent::text(filtered.content));
            }

            if !trailing_thinking.is_empty() {
                content.push(MessageContent::thinking(trailing_thinking, ""));
            }

            yield (
                Some(Message::new(
                    Role::Assistant,
                    chrono::Utc::now().timestamp(),
                    content,
                )),
                None,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_null_tool_call_arguments_as_empty_string() {
        let parsed: DeltaToolCallFunction = serde_json::from_str(r#"{"arguments":null}"#).unwrap();
        assert_eq!(parsed.arguments, "");
    }

    #[test]
    fn reasoning_text_prefers_reasoning_content() {
        let delta = Delta {
            reasoning: Some("fallback".to_string()),
            reasoning_content: Some("preferred".to_string()),
            ..Default::default()
        };

        assert_eq!(delta.reasoning_text(), Some("preferred"));
    }

    #[test]
    fn get_usage_reads_nested_usage_object() {
        let usage = get_usage(&json!({
            "id": "chatcmpl_test",
            "usage": {
                "prompt_tokens": 84,
                "completion_tokens": 21,
                "total_tokens": 105,
                "cache_read_input_tokens": 60,
                "cache_creation_input_tokens": 10
            }
        }));

        assert_eq!(usage.input_tokens, Some(84));
        assert_eq!(usage.output_tokens, Some(21));
        assert_eq!(usage.total_tokens, Some(105));
        assert_eq!(usage.cache_read_input_tokens, Some(60));
        assert_eq!(usage.cache_write_input_tokens, Some(10));
    }

    #[tokio::test]
    async fn streaming_response_extracts_inline_think_blocks() -> anyhow::Result<()> {
        use futures::{pin_mut, stream, StreamExt};

        let lines = [
            r#"data: {"id":"chunk-1","choices":[{"delta":{"content":"<thi"},"index":0,"finish_reason":null}]}"#,
            r#"data: {"id":"chunk-1","choices":[{"delta":{"content":"nk>x</thi"},"index":0,"finish_reason":null}]}"#,
            r#"data: {"id":"chunk-1","choices":[{"delta":{"content":"nk>y"},"index":0,"finish_reason":"stop"}]}"#,
            "data: [DONE]",
        ];
        let response_stream = stream::iter(lines.into_iter().map(|line| Ok(line.to_string())));
        let messages = response_to_streaming_message(response_stream);
        pin_mut!(messages);

        let mut text = String::new();
        let mut thinking = String::new();

        while let Some(result) = messages.next().await {
            let (message, _) = result?;
            if let Some(message) = message {
                for item in message.content {
                    match item {
                        MessageContent::Text(text_content) => text.push_str(&text_content.text),
                        MessageContent::Thinking(thinking_content) => {
                            thinking.push_str(&thinking_content.thinking)
                        }
                        _ => {}
                    }
                }
            }
        }

        assert_eq!(text, "y");
        assert_eq!(thinking, "x");

        Ok(())
    }

    #[tokio::test]
    async fn streaming_response_preserves_nested_tool_call_metadata() -> anyhow::Result<()> {
        use futures::{pin_mut, stream, StreamExt};

        let lines = [
            r#"data: {"model":"test-model","choices":[{"delta":{"role":"assistant","tool_calls":[{"extra_content":{"google":{"thought_signature":"nested_stream_sig"}},"id":"call_nested","function":{"name":"test_tool","arguments":"{}"},"type":"function","index":0}]},"index":0,"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":100,"completion_tokens":10,"total_tokens":110},"object":"chat.completion.chunk","id":"test-id","created":1234567890}"#,
            "data: [DONE]",
        ];
        let response_stream = stream::iter(lines.into_iter().map(|line| Ok(line.to_string())));
        let messages = response_to_streaming_message(response_stream);
        pin_mut!(messages);

        while let Some(result) = messages.next().await {
            let (message, _) = result?;
            if let Some(message) = message {
                if let MessageContent::ToolRequest(request) = &message.content[0] {
                    let metadata = request.metadata.as_ref().expect("metadata should exist");
                    assert_eq!(
                        metadata["extra_content"]["google"]["thought_signature"],
                        "nested_stream_sig"
                    );
                    return Ok(());
                }
            }
        }

        panic!("expected tool call message with nested metadata");
    }
}
