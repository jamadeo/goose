use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponsesApiResponse {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    pub status: String,
    pub model: String,
    pub output: Vec<ResponseOutputItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ResponseReasoningInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ResponseUsage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub struct SummaryText {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ResponseOutputItem {
    Reasoning {
        id: String,
        #[serde(default)]
        summary: Vec<SummaryText>,
    },
    Message {
        id: String,
        status: String,
        role: String,
        content: Vec<ResponseContentBlock>,
    },
    FunctionCall {
        id: String,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
        name: String,
        arguments: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ResponseContentBlock {
    OutputText {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: Option<Vec<Value>>,
    },
    Refusal {
        refusal: String,
    },
    ToolCall {
        id: String,
        name: String,
        input: Value,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseReasoningInfo {
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseUsage {
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub total_tokens: i32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ResponsesStreamEvent {
    #[serde(rename = "response.created")]
    ResponseCreated {
        sequence_number: i32,
        response: ResponseMetadata,
    },
    #[serde(rename = "response.in_progress")]
    ResponseInProgress {
        sequence_number: i32,
        response: ResponseMetadata,
    },
    #[serde(rename = "response.output_item.added")]
    OutputItemAdded {
        sequence_number: i32,
        output_index: i32,
        item: ResponseOutputItemInfo,
    },
    #[serde(rename = "response.content_part.added")]
    ContentPartAdded {
        sequence_number: i32,
        item_id: String,
        output_index: i32,
        content_index: i32,
        part: ContentPart,
    },
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta {
        sequence_number: i32,
        item_id: String,
        output_index: i32,
        content_index: i32,
        delta: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        logprobs: Option<Vec<Value>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        obfuscation: Option<String>,
    },
    #[serde(rename = "response.output_item.done")]
    OutputItemDone {
        sequence_number: i32,
        output_index: i32,
        item: ResponseOutputItemInfo,
    },
    #[serde(rename = "response.content_part.done")]
    ContentPartDone {
        sequence_number: i32,
        item_id: String,
        output_index: i32,
        content_index: i32,
        part: ContentPart,
    },
    #[serde(rename = "response.output_text.done")]
    OutputTextDone {
        sequence_number: i32,
        item_id: String,
        output_index: i32,
        content_index: i32,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        logprobs: Option<Vec<Value>>,
    },
    #[serde(rename = "response.completed")]
    ResponseCompleted {
        sequence_number: i32,
        response: ResponseMetadata,
    },
    #[serde(rename = "response.failed")]
    ResponseFailed { sequence_number: i32, error: Value },
    #[serde(rename = "response.function_call_arguments.delta")]
    FunctionCallArgumentsDelta {
        sequence_number: i32,
        item_id: String,
        output_index: i32,
        delta: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        obfuscation: Option<String>,
    },
    #[serde(rename = "response.function_call_arguments.done")]
    FunctionCallArgumentsDone {
        sequence_number: i32,
        item_id: String,
        output_index: i32,
        arguments: String,
    },
    #[serde(rename = "response.refusal.delta")]
    RefusalDelta {
        sequence_number: i32,
        item_id: String,
        output_index: i32,
        content_index: i32,
        delta: String,
    },
    #[serde(rename = "response.refusal.done")]
    RefusalDone {
        sequence_number: i32,
        item_id: String,
        output_index: i32,
        content_index: i32,
        refusal: String,
    },
    #[serde(rename = "error")]
    Error { error: Value },
    #[serde(rename = "keepalive")]
    Keepalive {
        #[serde(default)]
        sequence_number: Option<i32>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseMetadata {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    pub status: String,
    pub model: String,
    #[serde(default)]
    pub output: Vec<ResponseOutputItemInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ResponseUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ResponseReasoningInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ResponseOutputItemInfo {
    Reasoning {
        id: String,
        #[serde(default)]
        summary: Vec<SummaryText>,
    },
    Message {
        id: String,
        status: String,
        role: String,
        content: Vec<ContentPart>,
    },
    FunctionCall {
        id: String,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
        name: String,
        arguments: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ContentPart {
    OutputText {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: Option<Vec<Value>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        logprobs: Option<Vec<Value>>,
    },
    Refusal {
        refusal: String,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_reasoning_info_with_null_effort() {
        let info: ResponseReasoningInfo = serde_json::from_str(r#"{"effort": null}"#).unwrap();
        assert_eq!(info.effort, None);
        assert_eq!(info.summary, None);
    }

    #[test]
    fn deserializes_reasoning_info_with_effort() {
        let info: ResponseReasoningInfo =
            serde_json::from_str(r#"{"effort": "high", "summary": "Thought deeply"}"#).unwrap();
        assert_eq!(info.effort.as_deref(), Some("high"));
        assert_eq!(info.summary.as_deref(), Some("Thought deeply"));
    }

    #[test]
    fn deserializes_refusal_content_block() {
        let block: ResponseContentBlock = serde_json::from_str(
            r#"{"type":"refusal","refusal":"I cannot help with that request."}"#,
        )
        .unwrap();

        match block {
            ResponseContentBlock::Refusal { refusal } => {
                assert_eq!(refusal, "I cannot help with that request.");
            }
            _ => panic!("expected refusal block"),
        }
    }

    #[test]
    fn deserializes_refusal_content_part() {
        let item: ResponseOutputItemInfo = serde_json::from_str(
            r#"{
                "type": "message",
                "id": "msg_1",
                "status": "completed",
                "role": "assistant",
                "content": [{"type": "refusal", "refusal": "I'm unable to assist."}]
            }"#,
        )
        .unwrap();

        match item {
            ResponseOutputItemInfo::Message { content, .. } => {
                assert!(matches!(content[0], ContentPart::Refusal { .. }));
            }
            _ => panic!("expected message output item"),
        }
    }

    #[test]
    fn deserializes_refusal_delta_stream_event() {
        let event: ResponsesStreamEvent = serde_json::from_str(
            r#"{"type":"response.refusal.delta","sequence_number":5,"item_id":"msg_1","output_index":0,"content_index":0,"delta":"I cannot"}"#,
        )
        .unwrap();

        match event {
            ResponsesStreamEvent::RefusalDelta { delta, .. } => {
                assert_eq!(delta, "I cannot");
            }
            _ => panic!("expected RefusalDelta event"),
        }
    }
}
