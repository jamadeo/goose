pub use goose_types::{
    ActionRequired, ActionRequiredData, FrontendToolRequest, InferenceMetadata, Message,
    MessageContent, MessageMetadata, MessageProviderMetadata as ProviderMetadata,
    PersistedChainSummary, RedactedThinkingContent, SystemNotificationContent,
    SystemNotificationType, ThinkingContent, TokenState, ToolCallResult, ToolConfirmationRequest,
    ToolRequest, ToolResponse, ToolResult, TOOL_META_CHAIN_SUMMARY_KEY,
    TOOL_META_EXTERNAL_DISPATCH_KEY, TOOL_META_TITLE_KEY,
};

#[cfg(test)]
mod tests {
    use crate::conversation::message::{Message, MessageContent, MessageMetadata};
    use rmcp::model::{
        AnnotateAble, CallToolRequestParams, PromptMessage, PromptMessageContent,
        PromptMessageRole, RawEmbeddedResource, RawImageContent, ResourceContents, Role,
    };
    use rmcp::model::{ErrorCode, ErrorData};
    use rmcp::object;
    use serde_json::Value;

    #[test]
    fn test_sanitize_with_text() {
        let malicious = "Hello\u{E0041}\u{E0042}\u{E0043}world"; // Invisible "ABC"
        let message = Message::user().with_text(malicious);
        assert_eq!(message.as_concat_text(), "Helloworld");
    }

    #[test]
    fn test_no_sanitize_with_text() {
        let clean_text = "Hello world 世界 🌍";
        let message = Message::user().with_text(clean_text);
        assert_eq!(message.as_concat_text(), clean_text);
    }

    #[test]
    fn test_message_serialization() {
        let message = Message::assistant()
            .with_text("Hello, I'll help you with that.")
            .with_tool_request(
                "tool123",
                Ok(CallToolRequestParams::new("test_tool")
                    .with_arguments(object!({"param": "value"}))),
            );

        let json_str = serde_json::to_string_pretty(&message).unwrap();
        println!("Serialized message: {}", json_str);

        // Parse back to Value to check structure
        let value: Value = serde_json::from_str(&json_str).unwrap();

        // Check top-level fields
        assert_eq!(value["role"], "assistant");
        assert!(value["created"].is_i64());
        assert!(value["content"].is_array());

        // Check content items
        let content = &value["content"];

        // First item should be text
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "Hello, I'll help you with that.");

        // Second item should be toolRequest
        assert_eq!(content[1]["type"], "toolRequest");
        assert_eq!(content[1]["id"], "tool123");

        // Check tool_call serialization
        assert_eq!(content[1]["toolCall"]["status"], "success");
        assert_eq!(content[1]["toolCall"]["value"]["name"], "test_tool");
        assert_eq!(
            content[1]["toolCall"]["value"]["arguments"]["param"],
            "value"
        );
    }

    #[test]
    fn test_error_serialization() {
        let message = Message::assistant().with_tool_request(
            "tool123",
            Err(ErrorData {
                code: ErrorCode::INTERNAL_ERROR,
                message: std::borrow::Cow::from("Something went wrong".to_string()),
                data: None,
            }),
        );

        let json_str = serde_json::to_string_pretty(&message).unwrap();
        println!("Serialized error: {}", json_str);

        // Parse back to Value to check structure
        let value: Value = serde_json::from_str(&json_str).unwrap();

        // Check tool_call serialization with error
        let tool_call = &value["content"][0]["toolCall"];
        assert_eq!(tool_call["status"], "error");
        assert_eq!(tool_call["error"], "-32603: Something went wrong");
    }

    #[test]
    fn test_deserialization() {
        // Create a JSON string with our new format
        let json_str = r#"{
            "role": "assistant",
            "created": 1740171566,
            "content": [
                {
                    "type": "text",
                    "text": "I'll help you with that."
                },
                {
                    "type": "toolRequest",
                    "id": "tool123",
                    "toolCall": {
                        "status": "success",
                        "value": {
                            "name": "test_tool",
                            "arguments": {"param": "value"}
                        }
                    }
                }
            ],
            "metadata": { "agentVisible": true, "userVisible": true }
        }"#;

        let message: Message = serde_json::from_str(json_str).unwrap();

        assert_eq!(message.role, Role::Assistant);
        assert_eq!(message.created, 1740171566);
        assert_eq!(message.content.len(), 2);

        // Check first content item
        if let MessageContent::Text(text) = &message.content[0] {
            assert_eq!(text.text, "I'll help you with that.");
        } else {
            panic!("Expected Text content");
        }

        // Check second content item
        if let MessageContent::ToolRequest(req) = &message.content[1] {
            assert_eq!(req.id, "tool123");
            if let Ok(tool_call) = &req.tool_call {
                assert_eq!(tool_call.name, "test_tool");
                assert_eq!(tool_call.arguments, Some(object!({"param": "value"})))
            } else {
                panic!("Expected successful tool call");
            }
        } else {
            panic!("Expected ToolRequest content");
        }
    }

    #[test]
    fn test_deserialization_migrates_reasoning_to_thinking() {
        let json = serde_json::json!({
            "role": "assistant",
            "created": 1740171566,
            "content": [
                { "type": "reasoning", "text": "step by step" },
                { "type": "text", "text": "final answer" }
            ],
            "metadata": { "agentVisible": true, "userVisible": true }
        });

        let message: Message = serde_json::from_value(json).unwrap();
        assert_eq!(message.content.len(), 2);

        let MessageContent::Thinking(thinking) = &message.content[0] else {
            panic!("Expected Thinking content");
        };
        assert_eq!(thinking.thinking, "step by step");
        assert!(thinking.signature.is_empty());
    }

    #[test]
    fn test_agent_visible_content_preserves_thinking_for_provider() {
        let message = Message::assistant()
            .with_thinking("internal reasoning", "sig")
            .with_redacted_thinking("redacted")
            .with_text("final answer");

        let provider_message = message.agent_visible_content();
        assert_eq!(provider_message.content.len(), 3);
        assert!(matches!(
            provider_message.content[0],
            MessageContent::Thinking(_)
        ));
        assert!(matches!(
            provider_message.content[1],
            MessageContent::RedactedThinking(_)
        ));
    }

    #[test]
    fn test_deserialization_drops_invalid_reasoning_blocks() {
        let json = serde_json::json!({
            "role": "assistant",
            "created": 1740171566,
            "content": [
                { "type": "reasoning" },
                { "type": "reasoning", "text": 42 },
                { "type": "text", "text": "still here" }
            ],
            "metadata": { "agentVisible": true, "userVisible": true }
        });

        let message: Message = serde_json::from_value(json).unwrap();
        assert_eq!(message.content.len(), 1);

        let MessageContent::Text(text) = &message.content[0] else {
            panic!("Expected Text content");
        };
        assert_eq!(text.text, "still here");
    }

    #[test]
    fn test_from_prompt_message_text() {
        let prompt_content = PromptMessageContent::Text {
            text: "Hello, world!".to_string(),
        };

        let prompt_message = PromptMessage::new(PromptMessageRole::User, prompt_content);

        let message = Message::from(prompt_message);

        if let MessageContent::Text(text_content) = &message.content[0] {
            assert_eq!(text_content.text, "Hello, world!");
        } else {
            panic!("Expected MessageContent::Text");
        }
    }

    #[test]
    fn test_from_prompt_message_image() {
        let prompt_content = PromptMessageContent::Image {
            image: RawImageContent {
                data: "base64data".to_string(),
                mime_type: "image/jpeg".to_string(),
                meta: None,
            }
            .no_annotation(),
        };

        let prompt_message = PromptMessage::new(PromptMessageRole::User, prompt_content);

        let message = Message::from(prompt_message);

        if let MessageContent::Image(image_content) = &message.content[0] {
            assert_eq!(image_content.data, "base64data");
            assert_eq!(image_content.mime_type, "image/jpeg");
        } else {
            panic!("Expected MessageContent::Image");
        }
    }

    #[test]
    fn test_from_prompt_message_text_resource() {
        let resource = ResourceContents::TextResourceContents {
            uri: "file:///test.txt".to_string(),
            mime_type: Some("text/plain".to_string()),
            text: "Resource content".to_string(),
            meta: None,
        };

        let prompt_content = PromptMessageContent::Resource {
            resource: RawEmbeddedResource {
                resource,
                meta: None,
            }
            .no_annotation(),
        };

        let prompt_message = PromptMessage::new(PromptMessageRole::User, prompt_content);

        let message = Message::from(prompt_message);

        if let MessageContent::Text(text_content) = &message.content[0] {
            assert_eq!(text_content.text, "Resource content");
        } else {
            panic!("Expected MessageContent::Text");
        }
    }

    #[test]
    fn test_from_prompt_message() {
        // Test user message conversion
        let prompt_message = PromptMessage::new(
            PromptMessageRole::User,
            PromptMessageContent::Text {
                text: "Hello, world!".to_string(),
            },
        );

        let message = Message::from(prompt_message);
        assert_eq!(message.role, Role::User);
        assert_eq!(message.content.len(), 1);
        assert_eq!(message.as_concat_text(), "Hello, world!");

        // Test assistant message conversion
        let prompt_message = PromptMessage::new(
            PromptMessageRole::Assistant,
            PromptMessageContent::Text {
                text: "I can help with that.".to_string(),
            },
        );

        let message = Message::from(prompt_message);
        assert_eq!(message.role, Role::Assistant);
        assert_eq!(message.content.len(), 1);
        assert_eq!(message.as_concat_text(), "I can help with that.");
    }

    #[test]
    fn test_message_with_text() {
        let message = Message::user().with_text("Hello");
        assert_eq!(message.as_concat_text(), "Hello");
    }

    #[test]
    fn test_message_with_tool_request() {
        let tool_call = Ok(CallToolRequestParams::new("test_tool").with_arguments(object!({})));

        let message = Message::assistant().with_tool_request("req1", tool_call);
        assert!(message.is_tool_call());
        assert!(!message.is_tool_response());

        let ids = message.get_tool_ids();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains("req1"));
    }

    #[test]
    fn test_message_deserialization_sanitizes_text_content() {
        // Create a test string with Unicode Tags characters
        let malicious_text = "Hello\u{E0041}\u{E0042}\u{E0043}world";
        let malicious_json = format!(
            r#"{{
            "id": "test-id",
            "role": "user",
            "created": 1640995200,
            "content": [
                {{
                    "type": "text",
                    "text": "{}"
                }},
                {{
                    "type": "image",
                    "data": "base64data",
                    "mimeType": "image/png"
                }}
            ],
            "metadata": {{ "agentVisible": true, "userVisible": true }}
        }}"#,
            malicious_text
        );

        let message: Message = serde_json::from_str(&malicious_json).unwrap();

        // Text content should be sanitized
        assert_eq!(message.as_concat_text(), "Helloworld");

        // Image content should be unchanged
        if let MessageContent::Image(img) = &message.content[1] {
            assert_eq!(img.data, "base64data");
            assert_eq!(img.mime_type, "image/png");
        } else {
            panic!("Expected ImageContent");
        }
    }

    #[test]
    fn test_legitimate_unicode_preserved_during_message_deserialization() {
        let clean_json = r#"{
            "id": "test-id",
            "role": "user",
            "created": 1640995200,
            "content": [{
                "type": "text",
                "text": "Hello world 世界 🌍"
            }],
            "metadata": { "agentVisible": true, "userVisible": true }
        }"#;

        let message: Message = serde_json::from_str(clean_json).unwrap();

        assert_eq!(message.as_concat_text(), "Hello world 世界 🌍");
    }

    #[test]
    fn test_message_metadata_defaults() {
        let message = Message::user().with_text("Test");

        // By default, messages should be both user and agent visible
        assert!(message.is_user_visible());
        assert!(message.is_agent_visible());
    }

    #[test]
    fn test_message_visibility_methods() {
        // Test user_only
        let user_only_msg = Message::user().with_text("User only").user_only();
        assert!(user_only_msg.is_user_visible());
        assert!(!user_only_msg.is_agent_visible());

        // Test agent_only
        let agent_only_msg = Message::assistant().with_text("Agent only").agent_only();
        assert!(!agent_only_msg.is_user_visible());
        assert!(agent_only_msg.is_agent_visible());

        // Test with_visibility
        let custom_msg = Message::user()
            .with_text("Custom visibility")
            .with_visibility(false, true);
        assert!(!custom_msg.is_user_visible());
        assert!(custom_msg.is_agent_visible());
    }

    #[test]
    fn test_message_metadata_serialization() {
        let message = Message::user()
            .with_text("Test message")
            .with_visibility(false, true);

        let json_str = serde_json::to_string(&message).unwrap();
        let value: Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(value["metadata"]["userVisible"], false);
        assert_eq!(value["metadata"]["agentVisible"], true);
    }

    #[test]
    fn test_message_metadata_deserialization() {
        // Test with explicit metadata
        let json_with_metadata = r#"{
            "role": "user",
            "created": 1640995200,
            "content": [{
                "type": "text",
                "text": "Test"
            }],
            "metadata": {
                "userVisible": false,
                "agentVisible": true
            }
        }"#;

        let message: Message = serde_json::from_str(json_with_metadata).unwrap();
        assert!(!message.is_user_visible());
        assert!(message.is_agent_visible());
    }

    #[test]
    fn test_message_metadata_static_methods() {
        // Test MessageMetadata::agent_only()
        let agent_only_metadata = MessageMetadata::agent_only();
        assert!(!agent_only_metadata.user_visible);
        assert!(agent_only_metadata.agent_visible);

        // Test MessageMetadata::user_only()
        let user_only_metadata = MessageMetadata::user_only();
        assert!(user_only_metadata.user_visible);
        assert!(!user_only_metadata.agent_visible);

        // Test MessageMetadata::invisible()
        let invisible_metadata = MessageMetadata::invisible();
        assert!(!invisible_metadata.user_visible);
        assert!(!invisible_metadata.agent_visible);

        // Test using them with messages
        let agent_msg = Message::assistant()
            .with_text("Agent only message")
            .with_metadata(MessageMetadata::agent_only());
        assert!(!agent_msg.is_user_visible());
        assert!(agent_msg.is_agent_visible());

        let user_msg = Message::user()
            .with_text("User only message")
            .with_metadata(MessageMetadata::user_only());
        assert!(user_msg.is_user_visible());
        assert!(!user_msg.is_agent_visible());

        let invisible_msg = Message::user()
            .with_text("Invisible message")
            .with_metadata(MessageMetadata::invisible());
        assert!(!invisible_msg.is_user_visible());
        assert!(!invisible_msg.is_agent_visible());
    }

    #[test]
    fn test_message_metadata_builder_methods() {
        // Test with_agent_invisible
        let metadata = MessageMetadata::default().with_agent_invisible();
        assert!(metadata.user_visible);
        assert!(!metadata.agent_visible);

        // Test with_user_invisible
        let metadata = MessageMetadata::default().with_user_invisible();
        assert!(!metadata.user_visible);
        assert!(metadata.agent_visible);

        // Test with_agent_visible
        let metadata = MessageMetadata::invisible().with_agent_visible();
        assert!(!metadata.user_visible);
        assert!(metadata.agent_visible);

        // Test with_user_visible
        let metadata = MessageMetadata::invisible().with_user_visible();
        assert!(metadata.user_visible);
        assert!(!metadata.agent_visible);

        // Test chaining
        let metadata = MessageMetadata::invisible()
            .with_user_visible()
            .with_agent_visible();
        assert!(metadata.user_visible);
        assert!(metadata.agent_visible);
    }

    #[test]
    fn test_legacy_tool_response_deserialization() {
        let legacy_json = r#"{
            "role": "user",
            "created": 1640995200,
            "content": [{
                "type": "toolResponse",
                "id": "tool123",
                "toolResult": {
                    "status": "success",
                    "value": [
                        {
                            "type": "text",
                            "text": "Tool output text"
                        }
                    ]
                }
            }],
            "metadata": { "agentVisible": true, "userVisible": true }
        }"#;

        let message: Message = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(message.content.len(), 1);

        if let MessageContent::ToolResponse(response) = &message.content[0] {
            assert_eq!(response.id, "tool123");
            if let Ok(result) = &response.tool_result {
                assert_eq!(result.content.len(), 1);
                assert_eq!(
                    result.content[0].as_text().unwrap().text,
                    "Tool output text"
                );
            } else {
                panic!("Expected successful tool result");
            }
        } else {
            panic!("Expected ToolResponse content");
        }
    }

    #[test]
    fn test_new_tool_response_deserialization() {
        let new_json = r#"{
            "role": "user",
            "created": 1640995200,
            "content": [{
                "type": "toolResponse",
                "id": "tool456",
                "toolResult": {
                    "status": "success",
                    "value": {
                        "content": [
                            {
                                "type": "text",
                                "text": "New format output"
                            }
                        ],
                        "isError": false
                    }
                }
            }],
            "metadata": { "agentVisible": true, "userVisible": true }
        }"#;

        let message: Message = serde_json::from_str(new_json).unwrap();
        assert_eq!(message.content.len(), 1);

        if let MessageContent::ToolResponse(response) = &message.content[0] {
            assert_eq!(response.id, "tool456");
            if let Ok(result) = &response.tool_result {
                assert_eq!(result.content.len(), 1);
                assert_eq!(
                    result.content[0].as_text().unwrap().text,
                    "New format output"
                );
            } else {
                panic!("Expected successful tool result");
            }
        } else {
            panic!("Expected ToolResponse content");
        }
    }

    #[test]
    fn test_tool_request_with_value_arguments_backward_compatibility() {
        struct TestCase {
            name: &'static str,
            arguments_json: &'static str,
            expected: Option<Value>,
        }

        let test_cases = [
            TestCase {
                name: "string",
                arguments_json: r#""string_argument""#,
                expected: Some(serde_json::json!({"value": "string_argument"})),
            },
            TestCase {
                name: "array",
                arguments_json: r#"["a", "b", "c"]"#,
                expected: Some(serde_json::json!({"value": ["a", "b", "c"]})),
            },
            TestCase {
                name: "number",
                arguments_json: "42",
                expected: Some(serde_json::json!({"value": 42})),
            },
            TestCase {
                name: "null",
                arguments_json: "null",
                expected: None,
            },
            TestCase {
                name: "object",
                arguments_json: r#"{"key": "value", "number": 123}"#,
                expected: Some(serde_json::json!({"key": "value", "number": 123})),
            },
        ];

        for tc in test_cases {
            let json = format!(
                r#"{{
                    "role": "assistant",
                    "created": 1640995200,
                    "content": [{{
                        "type": "toolRequest",
                        "id": "tool123",
                        "toolCall": {{
                            "status": "success",
                            "value": {{
                                "name": "test_tool",
                                "arguments": {}
                            }}
                        }}
                    }}],
                    "metadata": {{ "agentVisible": true, "userVisible": true }}
                }}"#,
                tc.arguments_json
            );

            let message: Message = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("{}: parse failed: {}", tc.name, e));

            let MessageContent::ToolRequest(request) = &message.content[0] else {
                panic!("{}: expected ToolRequest content", tc.name);
            };

            let Ok(tool_call) = &request.tool_call else {
                panic!("{}: expected successful tool call", tc.name);
            };

            assert_eq!(tool_call.name, "test_tool", "{}: wrong tool name", tc.name);

            match (&tool_call.arguments, &tc.expected) {
                (None, None) => {}
                (Some(args), Some(expected)) => {
                    let args_value = serde_json::to_value(args).unwrap();
                    assert_eq!(&args_value, expected, "{}: arguments mismatch", tc.name);
                }
                (actual, expected) => {
                    panic!("{}: expected {:?}, got {:?}", tc.name, expected, actual);
                }
            }
        }
    }

    fn make_tool_request(meta: Option<serde_json::Value>) -> super::ToolRequest {
        super::ToolRequest {
            id: "id-1".to_string(),
            tool_call: Ok(CallToolRequestParams::new("test_tool")),
            metadata: None,
            tool_meta: meta,
        }
    }

    #[test]
    fn persisted_title_returns_none_when_meta_missing() {
        let req = make_tool_request(None);
        assert_eq!(req.persisted_title(), None);
    }

    #[test]
    fn persisted_title_returns_value_when_present() {
        let meta = serde_json::json!({
            super::TOOL_META_TITLE_KEY: "reading project configuration",
        });
        let req = make_tool_request(Some(meta));
        assert_eq!(req.persisted_title(), Some("reading project configuration"));
    }

    #[test]
    fn persisted_title_returns_none_for_non_string_value() {
        let meta = serde_json::json!({ super::TOOL_META_TITLE_KEY: 42 });
        let req = make_tool_request(Some(meta));
        assert_eq!(req.persisted_title(), None);
    }

    #[test]
    fn persisted_title_does_not_collide_with_external_dispatch() {
        let meta = serde_json::json!({
            super::TOOL_META_EXTERNAL_DISPATCH_KEY: true,
            super::TOOL_META_TITLE_KEY: "running commands",
        });
        let req = make_tool_request(Some(meta));
        assert!(req.is_externally_dispatched());
        assert_eq!(req.persisted_title(), Some("running commands"));
    }

    #[test]
    fn persisted_chain_summary_round_trips() {
        let meta = serde_json::json!({
            super::TOOL_META_CHAIN_SUMMARY_KEY: {
                "summary": "applied dark mode polish",
                "count": 4,
            },
        });
        let req = make_tool_request(Some(meta));
        let summary = req.persisted_chain_summary().expect("summary present");
        assert_eq!(summary.summary, "applied dark mode polish");
        assert_eq!(summary.count, 4);
    }

    #[test]
    fn persisted_chain_summary_returns_none_for_missing_or_zero_count() {
        let req = make_tool_request(None);
        assert!(req.persisted_chain_summary().is_none());

        let meta_zero = serde_json::json!({
            super::TOOL_META_CHAIN_SUMMARY_KEY: { "summary": "x", "count": 0 },
        });
        let req_zero = make_tool_request(Some(meta_zero));
        assert!(req_zero.persisted_chain_summary().is_none());

        let meta_no_summary = serde_json::json!({
            super::TOOL_META_CHAIN_SUMMARY_KEY: { "count": 3 },
        });
        let req_no_summary = make_tool_request(Some(meta_no_summary));
        assert!(req_no_summary.persisted_chain_summary().is_none());
    }
}
