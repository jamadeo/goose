use goose::models::message::{Message, MessageContent, Role};
use serde::Serialize;

/// A wrapper struct for Message that implements Serialize
#[derive(Serialize)]
pub struct SerializableMessage<'a> {
    role: &'a str,
    created: i64,
    content: Vec<SerializableContent<'a>>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum SerializableContent<'a> {
    Text {
        text: String,
    },
    ToolRequest {
        tool_name: &'a str,
        arguments: &'a serde_json::Value,
    },
    ToolResponse {
        id: &'a str,
        tool_result: String,
    },
    Image {
        // data: &'a str, // Don't serialize image data until further discussion.
        mime_type: &'a str,
    },
}

impl<'a> From<&'a Message> for SerializableMessage<'a> {
    fn from(msg: &'a Message) -> Self {
        let role = match msg.role {
            Role::Assistant => "assistant",
            Role::User => "user",
        };

        let created = msg.created;

        // Process all content items
        let content: Vec<SerializableContent> = msg
            .content
            .iter()
            .map(|content| {
                match content {
                    MessageContent::Text(text) => SerializableContent::Text {
                        text: text.text.to_string(),
                    },
                    MessageContent::ToolRequest(req) => match &req.tool_call {
                        Ok(tool_call) => SerializableContent::ToolRequest {
                            tool_name: &tool_call.name,
                            arguments: &tool_call.arguments,
                        },
                        Err(e) => SerializableContent::Text {
                            text: format!("Tool Request Error: {}", e),
                        },
                    },
                    MessageContent::ToolResponse(resp) => SerializableContent::ToolResponse {
                        id: &resp.id,
                        tool_result: match &resp.tool_result {
                            Ok(content) => serde_json::to_string(content).unwrap_or_else(|e| {
                                format!("{{\"error\": \"Failed to serialize: {}\"}}", e)
                            }),
                            Err(e) => format!("{{\"error\": \"{}\"}}", e),
                        },
                    },
                    MessageContent::Image(img) => SerializableContent::Image {
                        // data: &img.data,
                        mime_type: &img.mime_type,
                    },
                }
            })
            .collect();

        SerializableMessage {
            role,
            created,
            content,
        }
    }
}
