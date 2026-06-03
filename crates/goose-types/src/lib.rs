mod tool_result_serde;

use base64::Engine;
use regex::{Regex, RegexBuilder};
use rmcp::model::{
    AnnotateAble, CallToolRequestParams, CallToolResult, Content, ErrorData, ImageContent,
    JsonObject, RawContent, RawImageContent, RawTextContent, Role, TextContent,
};
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::ops::{Add, AddAssign};
use std::str::FromStr;
use std::sync::OnceLock;
use utoipa::ToSchema;

pub const DEFAULT_CONTEXT_LIMIT: usize = 128_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingEffort {
    Off,
    Low,
    Medium,
    High,
    Max,
}

impl FromStr for ThinkingEffort {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "off" | "disabled" | "none" => Ok(Self::Off),
            "low" => Ok(Self::Low),
            "medium" | "med" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "max" | "xhigh" => Ok(Self::Max),
            other => Err(format!("unknown thinking effort: '{other}'")),
        }
    }
}

impl fmt::Display for ThinkingEffort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => write!(f, "off"),
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Max => write!(f, "max"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ThinkingContent {
    pub thinking: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct RedactedThinkingContent {
    pub data: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InferenceMetadata {
    pub provider: String,
    pub requested_model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MessageMetadata {
    pub user_visible: bool,
    pub agent_visible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inference: Option<InferenceMetadata>,
}

impl Default for MessageMetadata {
    fn default() -> Self {
        Self {
            user_visible: true,
            agent_visible: true,
            inference: None,
        }
    }
}

pub type MessageProviderMetadata = serde_json::Map<String, serde_json::Value>;

pub type ToolResult<T> = Result<T, ErrorData>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum SystemNotificationType {
    ThinkingMessage,
    InlineMessage,
    CreditsExhausted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SystemNotificationContent {
    pub notification_type: SystemNotificationType,
    pub msg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenState {
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub total_tokens: i32,
    pub accumulated_input_tokens: i32,
    pub accumulated_output_tokens: i32,
    pub accumulated_total_tokens: i32,
    pub accumulated_cost: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum ToolCallResult<T> {
    Success { value: T },
    Error { error: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequest {
    pub id: String,
    #[serde(with = "tool_result_serde")]
    #[schema(value_type = Object)]
    pub tool_call: ToolResult<CallToolRequestParams>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object)]
    pub metadata: Option<MessageProviderMetadata>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object)]
    pub tool_meta: Option<serde_json::Value>,
}

impl ToolRequest {
    pub fn to_readable_string(&self) -> String {
        match &self.tool_call {
            Ok(tool_call) => {
                format!(
                    "Tool: {}, Args: {}",
                    tool_call.name,
                    serde_json::to_string_pretty(&tool_call.arguments)
                        .unwrap_or_else(|_| "<<invalid json>>".to_string())
                )
            }
            Err(e) => format!("Invalid tool call: {}", e),
        }
    }

    pub fn is_externally_dispatched(&self) -> bool {
        self.tool_meta
            .as_ref()
            .and_then(|v| v.get(TOOL_META_EXTERNAL_DISPATCH_KEY))
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    pub fn persisted_title(&self) -> Option<&str> {
        self.tool_meta
            .as_ref()
            .and_then(|v| v.get(TOOL_META_TITLE_KEY))
            .and_then(|v| v.as_str())
    }

    pub fn persisted_chain_summary(&self) -> Option<PersistedChainSummary> {
        let obj = self
            .tool_meta
            .as_ref()
            .and_then(|v| v.get(TOOL_META_CHAIN_SUMMARY_KEY))?;
        let summary = obj.get("summary").and_then(|v| v.as_str())?.to_string();
        let count = obj.get("count").and_then(|v| v.as_u64())?;
        if count == 0 {
            return None;
        }
        Some(PersistedChainSummary {
            summary,
            count: count as usize,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersistedChainSummary {
    pub summary: String,
    pub count: usize,
}

pub const TOOL_META_EXTERNAL_DISPATCH_KEY: &str = "goose.external_dispatch";
pub const TOOL_META_TITLE_KEY: &str = "goose.toolSummary.title";
pub const TOOL_META_CHAIN_SUMMARY_KEY: &str = "goose.toolChain.summary";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolResponse {
    pub id: String,
    #[serde(with = "tool_result_serde::call_tool_result")]
    #[schema(value_type = Object)]
    pub tool_result: ToolResult<CallToolResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object)]
    pub metadata: Option<MessageProviderMetadata>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolConfirmationRequest {
    pub id: String,
    pub tool_name: String,
    pub arguments: JsonObject,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrontendToolRequest {
    pub id: String,
    #[serde(with = "tool_result_serde")]
    #[schema(value_type = Object)]
    pub tool_call: ToolResult<CallToolRequestParams>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "actionType", rename_all = "camelCase")]
pub enum ActionRequiredData {
    #[serde(rename_all = "camelCase")]
    ToolConfirmation {
        id: String,
        tool_name: String,
        arguments: JsonObject,
        prompt: Option<String>,
    },
    Elicitation {
        id: String,
        message: String,
        requested_schema: serde_json::Value,
    },
    ElicitationResponse {
        id: String,
        user_data: serde_json::Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActionRequired {
    pub data: ActionRequiredData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
/// Content passed inside a message, which can be both simple content and tool content
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MessageContent {
    Text(TextContent),
    Image(ImageContent),
    ToolRequest(ToolRequest),
    ToolResponse(ToolResponse),
    ToolConfirmationRequest(ToolConfirmationRequest),
    ActionRequired(ActionRequired),
    FrontendToolRequest(FrontendToolRequest),
    Thinking(ThinkingContent),
    RedactedThinking(RedactedThinkingContent),
    SystemNotification(SystemNotificationContent),
}

impl fmt::Display for MessageContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageContent::Text(t) => write!(f, "{}", t.text),
            MessageContent::Image(i) => write!(f, "[Image: {}]", i.mime_type),
            MessageContent::ToolRequest(r) => {
                write!(f, "[ToolRequest: {}]", r.to_readable_string())
            }
            MessageContent::ToolResponse(r) => write!(
                f,
                "[ToolResponse: {}]",
                match &r.tool_result {
                    Ok(result) => format!("{} content item(s)", result.content.len()),
                    Err(e) => format!("Error: {e}"),
                }
            ),
            MessageContent::ToolConfirmationRequest(r) => {
                write!(f, "[ToolConfirmationRequest: {}]", r.tool_name)
            }
            MessageContent::ActionRequired(a) => match &a.data {
                ActionRequiredData::ToolConfirmation { tool_name, .. } => {
                    write!(f, "[ActionRequired: ToolConfirmation for {}]", tool_name)
                }
                ActionRequiredData::Elicitation { message, .. } => {
                    write!(f, "[ActionRequired: Elicitation - {}]", message)
                }
                ActionRequiredData::ElicitationResponse { id, .. } => {
                    write!(f, "[ActionRequired: ElicitationResponse for {}]", id)
                }
            },
            MessageContent::FrontendToolRequest(r) => match &r.tool_call {
                Ok(tool_call) => write!(f, "[FrontendToolRequest: {}]", tool_call.name),
                Err(e) => write!(f, "[FrontendToolRequest: Error: {}]", e),
            },
            MessageContent::Thinking(t) => write!(f, "[Thinking: {}]", t.thinking),
            MessageContent::RedactedThinking(_r) => write!(f, "[RedactedThinking]"),
            MessageContent::SystemNotification(r) => {
                write!(f, "[SystemNotification: {}]", r.msg)
            }
        }
    }
}

impl MessageContent {
    pub fn text<S: Into<String>>(text: S) -> Self {
        MessageContent::Text(
            RawTextContent {
                text: text.into(),
                meta: None,
            }
            .no_annotation(),
        )
    }

    pub fn filter_for_audience(&self, audience: Role) -> Option<MessageContent> {
        match self {
            MessageContent::Text(text) => {
                if text
                    .audience()
                    .map(|roles| roles.contains(&audience))
                    .unwrap_or(true)
                {
                    Some(self.clone())
                } else {
                    None
                }
            }
            MessageContent::Image(img) => {
                if img
                    .audience()
                    .map(|roles| roles.contains(&audience))
                    .unwrap_or(true)
                {
                    Some(self.clone())
                } else {
                    None
                }
            }
            MessageContent::ToolResponse(res) => {
                let Ok(result) = &res.tool_result else {
                    return Some(self.clone());
                };

                let filtered_content: Vec<Content> = result
                    .content
                    .iter()
                    .filter(|c| {
                        c.audience()
                            .map(|roles| roles.contains(&audience))
                            .unwrap_or(true)
                    })
                    .cloned()
                    .collect();

                // Preserve ToolResponse even when content is empty - some providers
                // (like Google) need to handle empty tool responses specially
                let mut tool_result = result.clone();
                tool_result.content = filtered_content;
                Some(MessageContent::ToolResponse(ToolResponse {
                    id: res.id.clone(),
                    tool_result: Ok(tool_result),
                    metadata: res.metadata.clone(),
                }))
            }
            MessageContent::Thinking(_) | MessageContent::RedactedThinking(_) => {
                if audience == Role::Assistant {
                    Some(self.clone())
                } else {
                    None
                }
            }
            _ => Some(self.clone()),
        }
    }

    pub fn image<S: Into<String>, T: Into<String>>(data: S, mime_type: T) -> Self {
        MessageContent::Image(
            RawImageContent {
                data: data.into(),
                mime_type: mime_type.into(),
                meta: None,
            }
            .no_annotation(),
        )
    }

    pub fn tool_request<S: Into<String>>(
        id: S,
        tool_call: ToolResult<CallToolRequestParams>,
    ) -> Self {
        MessageContent::ToolRequest(ToolRequest {
            id: id.into(),
            tool_call,
            metadata: None,
            tool_meta: None,
        })
    }

    pub fn tool_request_with_metadata<S: Into<String>>(
        id: S,
        tool_call: ToolResult<CallToolRequestParams>,
        metadata: Option<&MessageProviderMetadata>,
    ) -> Self {
        MessageContent::ToolRequest(ToolRequest {
            id: id.into(),
            tool_call,
            metadata: metadata.cloned(),
            tool_meta: None,
        })
    }

    pub fn tool_response<S: Into<String>>(id: S, tool_result: ToolResult<CallToolResult>) -> Self {
        MessageContent::ToolResponse(ToolResponse {
            id: id.into(),
            tool_result,
            metadata: None,
        })
    }

    pub fn tool_response_with_metadata<S: Into<String>>(
        id: S,
        tool_result: ToolResult<CallToolResult>,
        metadata: Option<&MessageProviderMetadata>,
    ) -> Self {
        MessageContent::ToolResponse(ToolResponse {
            id: id.into(),
            tool_result,
            metadata: metadata.cloned(),
        })
    }

    pub fn action_required<S: Into<String>>(
        id: S,
        tool_name: String,
        arguments: JsonObject,
        prompt: Option<String>,
    ) -> Self {
        MessageContent::ActionRequired(ActionRequired {
            data: ActionRequiredData::ToolConfirmation {
                id: id.into(),
                tool_name,
                arguments,
                prompt,
            },
        })
    }

    pub fn action_required_elicitation<S: Into<String>>(
        id: S,
        message: String,
        requested_schema: serde_json::Value,
    ) -> Self {
        MessageContent::ActionRequired(ActionRequired {
            data: ActionRequiredData::Elicitation {
                id: id.into(),
                message,
                requested_schema,
            },
        })
    }

    pub fn action_required_elicitation_response<S: Into<String>>(
        id: S,
        user_data: serde_json::Value,
    ) -> Self {
        MessageContent::ActionRequired(ActionRequired {
            data: ActionRequiredData::ElicitationResponse {
                id: id.into(),
                user_data,
            },
        })
    }

    pub fn thinking<S1: Into<String>, S2: Into<String>>(thinking: S1, signature: S2) -> Self {
        MessageContent::Thinking(ThinkingContent {
            thinking: thinking.into(),
            signature: signature.into(),
        })
    }

    pub fn redacted_thinking<S: Into<String>>(data: S) -> Self {
        MessageContent::RedactedThinking(RedactedThinkingContent { data: data.into() })
    }

    pub fn frontend_tool_request<S: Into<String>>(
        id: S,
        tool_call: ToolResult<CallToolRequestParams>,
    ) -> Self {
        MessageContent::FrontendToolRequest(FrontendToolRequest {
            id: id.into(),
            tool_call,
        })
    }

    pub fn system_notification<S: Into<String>>(
        notification_type: SystemNotificationType,
        msg: S,
    ) -> Self {
        MessageContent::SystemNotification(SystemNotificationContent {
            notification_type,
            msg: msg.into(),
            data: None,
        })
    }

    pub fn system_notification_with_data<S: Into<String>>(
        notification_type: SystemNotificationType,
        msg: S,
        data: serde_json::Value,
    ) -> Self {
        MessageContent::SystemNotification(SystemNotificationContent {
            notification_type,
            msg: msg.into(),
            data: Some(data),
        })
    }

    pub fn as_system_notification(&self) -> Option<&SystemNotificationContent> {
        if let MessageContent::SystemNotification(ref notification) = self {
            Some(notification)
        } else {
            None
        }
    }

    pub fn as_tool_request(&self) -> Option<&ToolRequest> {
        if let MessageContent::ToolRequest(ref tool_request) = self {
            Some(tool_request)
        } else {
            None
        }
    }

    pub fn as_tool_response(&self) -> Option<&ToolResponse> {
        if let MessageContent::ToolResponse(ref tool_response) = self {
            Some(tool_response)
        } else {
            None
        }
    }

    pub fn as_action_required(&self) -> Option<&ActionRequired> {
        if let MessageContent::ActionRequired(ref action_required) = self {
            Some(action_required)
        } else {
            None
        }
    }

    pub fn as_tool_response_text(&self) -> Option<String> {
        if let Some(tool_response) = self.as_tool_response() {
            if let Ok(result) = &tool_response.tool_result {
                let texts: Vec<String> = result
                    .content
                    .iter()
                    .filter_map(|content| content.as_text().map(|t| t.text.to_string()))
                    .collect();
                if !texts.is_empty() {
                    return Some(texts.join("\n"));
                }
            }
        }
        None
    }

    /// Get the text content if this is a TextContent variant
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MessageContent::Text(text) => Some(&text.text),
            _ => None,
        }
    }

    /// Get the thinking content if this is a ThinkingContent variant
    pub fn as_thinking(&self) -> Option<&ThinkingContent> {
        match self {
            MessageContent::Thinking(thinking) => Some(thinking),
            _ => None,
        }
    }

    /// Get the redacted thinking content if this is a RedactedThinkingContent variant
    pub fn as_redacted_thinking(&self) -> Option<&RedactedThinkingContent> {
        match self {
            MessageContent::RedactedThinking(redacted) => Some(redacted),
            _ => None,
        }
    }
}

impl From<Content> for MessageContent {
    fn from(content: Content) -> Self {
        match content.raw {
            RawContent::Text(text) => {
                MessageContent::Text(text.optional_annotate(content.annotations))
            }
            RawContent::Image(image) => {
                MessageContent::Image(image.optional_annotate(content.annotations))
            }
            RawContent::ResourceLink(_link) => MessageContent::text("[Resource link]"),
            RawContent::Resource(resource) => {
                MessageContent::text(extract_text_from_resource(&resource.resource))
            }
            RawContent::Audio(_) => {
                MessageContent::text("[Audio content: not supported]".to_string())
            }
        }
    }
}

pub fn extract_text_from_resource(resource: &rmcp::model::ResourceContents) -> String {
    match resource {
        rmcp::model::ResourceContents::TextResourceContents { text, .. } => text.clone(),
        rmcp::model::ResourceContents::BlobResourceContents {
            blob, mime_type, ..
        } => match base64::engine::general_purpose::STANDARD.decode(blob) {
            Ok(bytes) => {
                let byte_len = bytes.len();
                match String::from_utf8(bytes) {
                    Ok(text) => text,
                    Err(_) => {
                        let mime = mime_type
                            .as_ref()
                            .map(|m| m.as_str())
                            .unwrap_or("application/octet-stream");
                        format!("[Binary content ({}) - {} bytes]", mime, byte_len)
                    }
                }
            }
            Err(_) => blob.clone(),
        },
    }
}

impl MessageMetadata {
    pub fn agent_only() -> Self {
        Self {
            user_visible: false,
            agent_visible: true,
            ..Default::default()
        }
    }

    pub fn user_only() -> Self {
        Self {
            user_visible: true,
            agent_visible: false,
            ..Default::default()
        }
    }

    pub fn invisible() -> Self {
        Self {
            user_visible: false,
            agent_visible: false,
            ..Default::default()
        }
    }

    pub fn with_agent_invisible(self) -> Self {
        Self {
            agent_visible: false,
            ..self
        }
    }

    pub fn with_user_invisible(self) -> Self {
        Self {
            user_visible: false,
            ..self
        }
    }

    pub fn with_agent_visible(self) -> Self {
        Self {
            agent_visible: true,
            ..self
        }
    }

    pub fn with_user_visible(self) -> Self {
        Self {
            user_visible: true,
            ..self
        }
    }

    pub fn with_inference(mut self, inference: InferenceMetadata) -> Self {
        self.inference = Some(inference);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderUsage {
    pub model: String,
    pub usage: Usage,
}

impl ProviderUsage {
    pub fn new(model: String, usage: Usage) -> Self {
        Self { model, usage }
    }

    pub fn combine_with(&self, other: &ProviderUsage) -> ProviderUsage {
        ProviderUsage {
            model: self.model.clone(),
            usage: self.usage + other.usage,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Copy)]
pub struct Usage {
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
    pub total_tokens: Option<i32>,
    pub cache_read_input_tokens: Option<i32>,
    pub cache_write_input_tokens: Option<i32>,
}

fn sum_optionals<T>(a: Option<T>, b: Option<T>) -> Option<T>
where
    T: Add<Output = T> + Default,
{
    match (a, b) {
        (Some(x), Some(y)) => Some(x + y),
        (Some(x), None) => Some(x + T::default()),
        (None, Some(y)) => Some(T::default() + y),
        (None, None) => None,
    }
}

impl Add for Usage {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self::new(
            sum_optionals(self.input_tokens, other.input_tokens),
            sum_optionals(self.output_tokens, other.output_tokens),
            sum_optionals(self.total_tokens, other.total_tokens),
        )
        .with_cache_tokens(
            sum_optionals(self.cache_read_input_tokens, other.cache_read_input_tokens),
            sum_optionals(
                self.cache_write_input_tokens,
                other.cache_write_input_tokens,
            ),
        )
    }
}

impl AddAssign for Usage {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Usage {
    pub fn new(
        input_tokens: Option<i32>,
        output_tokens: Option<i32>,
        total_tokens: Option<i32>,
    ) -> Self {
        let calculated_total = if total_tokens.is_none() {
            match (input_tokens, output_tokens) {
                (Some(input), Some(output)) => Some(input + output),
                (Some(input), None) => Some(input),
                (None, Some(output)) => Some(output),
                (None, None) => None,
            }
        } else {
            total_tokens
        };

        Self {
            input_tokens,
            output_tokens,
            total_tokens: calculated_total,
            cache_read_input_tokens: None,
            cache_write_input_tokens: None,
        }
    }

    pub fn with_cache_tokens(
        mut self,
        cache_read_input_tokens: Option<i32>,
        cache_write_input_tokens: Option<i32>,
    ) -> Self {
        self.cache_read_input_tokens = cache_read_input_tokens;
        self.cache_write_input_tokens = cache_write_input_tokens;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigKey {
    pub name: String,
    pub required: bool,
    pub secret: bool,
    pub default: Option<String>,
    pub oauth_flow: bool,
    #[serde(default)]
    pub device_code_flow: bool,
    #[serde(default)]
    pub primary: bool,
}

impl ConfigKey {
    pub fn new(
        name: &str,
        required: bool,
        secret: bool,
        default: Option<&str>,
        primary: bool,
    ) -> Self {
        Self {
            name: name.to_string(),
            required,
            secret,
            default: default.map(|s| s.to_string()),
            oauth_flow: false,
            device_code_flow: false,
            primary,
        }
    }

    pub fn new_oauth(
        name: &str,
        required: bool,
        secret: bool,
        default: Option<&str>,
        primary: bool,
    ) -> Self {
        Self {
            name: name.to_string(),
            required,
            secret,
            default: default.map(|s| s.to_string()),
            oauth_flow: true,
            device_code_flow: false,
            primary,
        }
    }

    pub fn new_oauth_device_code(
        name: &str,
        required: bool,
        secret: bool,
        default: Option<&str>,
        primary: bool,
    ) -> Self {
        Self {
            name: name.to_string(),
            required,
            secret,
            default: default.map(|s| s.to_string()),
            oauth_flow: true,
            device_code_flow: true,
            primary,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct ModelInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_model: Option<String>,
    pub context_limit: usize,
    pub input_token_cost: Option<f64>,
    pub output_token_cost: Option<f64>,
    pub currency: Option<String>,
    pub supports_cache_control: Option<bool>,
    #[serde(default)]
    pub reasoning: bool,
}

impl ModelInfo {
    pub fn new(name: impl Into<String>, context_limit: usize) -> Self {
        Self {
            name: name.into(),
            resolved_model: None,
            context_limit,
            input_token_cost: None,
            output_token_cost: None,
            currency: None,
            supports_cache_control: None,
            reasoning: false,
        }
    }

    pub fn with_cost(
        name: impl Into<String>,
        context_limit: usize,
        input_cost: f64,
        output_cost: f64,
    ) -> Self {
        Self {
            name: name.into(),
            resolved_model: None,
            context_limit,
            input_token_cost: Some(input_cost),
            output_token_cost: Some(output_cost),
            currency: Some("$".to_string()),
            supports_cache_control: None,
            reasoning: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum ProviderType {
    Preferred,
    Builtin,
    Declarative,
    Custom,
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
pub struct ModelConfig {
    pub model_name: String,
    pub context_limit: Option<usize>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    pub toolshim: bool,
    pub toolshim_model: Option<String>,
    #[serde(skip)]
    pub fast_model_config: Option<Box<ModelConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_params: Option<HashMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
}

impl<'de> Deserialize<'de> for ModelConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawModelConfig {
            model_name: String,
            context_limit: Option<usize>,
            temperature: Option<f32>,
            max_tokens: Option<i32>,
            toolshim: bool,
            toolshim_model: Option<String>,
            #[serde(default)]
            fast_model_config: Option<Box<ModelConfig>>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            request_params: Option<HashMap<String, Value>>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            reasoning: Option<bool>,
        }

        let raw = RawModelConfig::deserialize(deserializer)?;
        let mut config = Self {
            model_name: raw.model_name,
            context_limit: raw.context_limit,
            temperature: raw.temperature,
            max_tokens: raw.max_tokens,
            toolshim: raw.toolshim,
            toolshim_model: raw.toolshim_model,
            fast_model_config: raw.fast_model_config,
            request_params: raw.request_params,
            reasoning: raw.reasoning,
        };
        config.normalize_effort_suffix();
        Ok(config)
    }
}

impl ModelConfig {
    pub fn with_context_limit(mut self, limit: Option<usize>) -> Self {
        if limit.is_some() {
            self.context_limit = limit;
        }
        self
    }

    pub fn with_temperature(mut self, temp: Option<f32>) -> Self {
        self.temperature = temp;
        self
    }

    pub fn with_max_tokens(mut self, tokens: Option<i32>) -> Self {
        self.max_tokens = tokens;
        self
    }

    pub fn with_toolshim(mut self, toolshim: bool) -> Self {
        self.toolshim = toolshim;
        self
    }

    pub fn with_toolshim_model(mut self, model: Option<String>) -> Self {
        self.toolshim_model = model;
        self
    }

    pub fn with_merged_request_params(mut self, params: HashMap<String, Value>) -> Self {
        match self.request_params.as_mut() {
            Some(existing) => {
                for (k, v) in params {
                    existing.insert(k, v);
                }
            }
            None => {
                self.request_params = Some(params);
            }
        }
        self
    }

    pub fn use_fast_model(&self) -> Self {
        if let Some(fast_config) = &self.fast_model_config {
            *fast_config.clone()
        } else {
            self.clone()
        }
    }

    pub fn context_limit(&self) -> usize {
        self.context_limit.unwrap_or(DEFAULT_CONTEXT_LIMIT)
    }

    pub fn is_openai_reasoning_model(&self) -> bool {
        is_openai_responses_model(&self.model_name)
    }

    pub fn is_reasoning_model(&self) -> bool {
        if let Some(reasoning) = self.reasoning {
            return reasoning;
        }

        self.is_openai_reasoning_model()
            || self.model_name.to_lowercase().contains("claude")
            || is_gemini3_reasoning_model_name(&self.model_name)
    }

    pub fn max_output_tokens(&self) -> i32 {
        self.max_tokens.unwrap_or(4_096)
    }

    pub fn normalize_effort_suffix(&mut self) {
        if !self.is_openai_reasoning_model() {
            return;
        }
        let parts: Vec<&str> = self.model_name.split('-').collect();
        let last = match parts.last() {
            Some(l) => *l,
            None => return,
        };
        let effort = match last {
            "none" => ThinkingEffort::Off,
            "low" => ThinkingEffort::Low,
            "medium" => ThinkingEffort::Medium,
            "high" => ThinkingEffort::High,
            "xhigh" => ThinkingEffort::Max,
            _ => return,
        };
        self.model_name = parts[..parts.len() - 1].join("-");
        let has_explicit_effort = self
            .request_params
            .as_ref()
            .and_then(|p| p.get("thinking_effort"))
            .is_some();
        if !has_explicit_effort {
            let params = self.request_params.get_or_insert_with(HashMap::new);
            params.insert(
                "thinking_effort".to_string(),
                serde_json::json!(effort.to_string()),
            );
        }
    }
}

pub fn is_openai_responses_model(model_name: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        RegexBuilder::new(r"(?:^|[-/])(?:o[0-9]+(?:$|-)|gpt-5(?:$|[-.]))")
            .case_insensitive(true)
            .unicode(false)
            .build()
            .unwrap()
    });
    re.is_match(model_name)
}

pub fn extract_reasoning_effort(model_name: &str) -> (String, Option<String>) {
    if !is_openai_responses_model(model_name) {
        return (model_name.to_string(), None);
    }

    let lower = model_name.to_ascii_lowercase();
    for effort in ["none", "low", "medium", "high", "xhigh"] {
        let suffix = format!("-{effort}");
        if lower.ends_with(&suffix) {
            let base = model_name
                .chars()
                .take(model_name.chars().count() - suffix.chars().count())
                .collect();
            return (base, Some(effort.to_string()));
        }
    }

    (model_name.to_string(), None)
}

pub fn openai_reasoning_effort_for_thinking(
    model_name: &str,
    effort: ThinkingEffort,
) -> Option<String> {
    if effort == ThinkingEffort::Off {
        return Some("none".to_string());
    }

    let supported = openai_reasoning_efforts_for_model(model_name);
    let preferred: &[&str] = match effort {
        ThinkingEffort::Off => unreachable!(),
        ThinkingEffort::Low => &["low", "medium", "high", "xhigh"],
        ThinkingEffort::Medium => &["medium", "high", "low", "xhigh"],
        ThinkingEffort::High => &["high", "medium", "xhigh", "low"],
        ThinkingEffort::Max => &["xhigh", "high", "medium", "low"],
    };

    preferred
        .iter()
        .find(|level| supported.contains(level))
        .map(|level| (*level).to_string())
}

fn is_gemini3_reasoning_model_name(model_name: &str) -> bool {
    let lower = model_name.to_lowercase();
    lower.starts_with("gemini-3") || lower.contains("/gemini-3") || lower.contains("-gemini-3")
}

fn openai_reasoning_efforts_for_model(model_name: &str) -> &'static [&'static str] {
    let normalized = model_name.to_ascii_lowercase();

    if normalized.contains("gpt-5") {
        if normalized.contains("-pro") || normalized.contains("/pro") {
            &["high"]
        } else if normalized.contains("gpt-5.4")
            || normalized.contains("gpt-5-4")
            || normalized.contains("gpt-5.5")
            || normalized.contains("gpt-5-5")
        {
            &["low", "medium", "high", "xhigh"]
        } else {
            &["low", "medium", "high"]
        }
    } else {
        &["low", "medium", "high"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool_request(meta: Option<serde_json::Value>) -> ToolRequest {
        ToolRequest {
            id: "id-1".to_string(),
            tool_call: Ok(CallToolRequestParams::new("test_tool")),
            metadata: None,
            tool_meta: meta,
        }
    }

    #[test]
    fn tool_request_deserializes_legacy_value_arguments() {
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
                    "id": "tool123",
                    "toolCall": {{
                        "status": "success",
                        "value": {{
                            "name": "test_tool",
                            "arguments": {}
                        }}
                    }}
                }}"#,
                tc.arguments_json
            );

            let request: ToolRequest = serde_json::from_str(&json).unwrap();
            let tool_call = request.tool_call.unwrap();

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

    #[test]
    fn tool_response_deserializes_legacy_content_vec() {
        let json = r#"{
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
        }"#;

        let response: ToolResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "tool123");
        let result = response.tool_result.unwrap();
        assert_eq!(result.content.len(), 1);
        assert_eq!(
            result.content[0].as_text().unwrap().text,
            "Tool output text"
        );
    }

    #[test]
    fn tool_response_deserializes_call_tool_result() {
        let json = r#"{
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
        }"#;

        let response: ToolResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "tool456");
        let result = response.tool_result.unwrap();
        assert_eq!(result.content.len(), 1);
        assert_eq!(
            result.content[0].as_text().unwrap().text,
            "New format output"
        );
    }

    #[test]
    fn tool_request_persisted_metadata_helpers() {
        let req = make_tool_request(None);
        assert_eq!(req.persisted_title(), None);
        assert!(req.persisted_chain_summary().is_none());

        let meta = serde_json::json!({
            TOOL_META_EXTERNAL_DISPATCH_KEY: true,
            TOOL_META_TITLE_KEY: "running commands",
            TOOL_META_CHAIN_SUMMARY_KEY: {
                "summary": "applied dark mode polish",
                "count": 4,
            },
        });
        let req = make_tool_request(Some(meta));
        assert!(req.is_externally_dispatched());
        assert_eq!(req.persisted_title(), Some("running commands"));
        let summary = req.persisted_chain_summary().expect("summary present");
        assert_eq!(summary.summary, "applied dark mode polish");
        assert_eq!(summary.count, 4);

        let req_zero = make_tool_request(Some(serde_json::json!({
            TOOL_META_CHAIN_SUMMARY_KEY: { "summary": "x", "count": 0 },
        })));
        assert!(req_zero.persisted_chain_summary().is_none());

        let req_no_summary = make_tool_request(Some(serde_json::json!({
            TOOL_META_CHAIN_SUMMARY_KEY: { "count": 3 },
        })));
        assert!(req_no_summary.persisted_chain_summary().is_none());
    }

    #[test]
    fn message_content_builders_create_text_image_and_thinking() {
        let text = MessageContent::text("hello");
        assert_eq!(text.as_text(), Some("hello"));

        let image = MessageContent::image("image-bytes", "image/png");
        match image {
            MessageContent::Image(image) => {
                assert_eq!(image.data, "image-bytes");
                assert_eq!(image.mime_type, "image/png");
            }
            other => panic!("expected image content, got {other:?}"),
        }

        let thinking = MessageContent::thinking("reasoning", "sig");
        assert_eq!(thinking.as_thinking().unwrap().thinking, "reasoning");
        assert_eq!(thinking.as_thinking().unwrap().signature, "sig");

        let redacted = MessageContent::redacted_thinking("redacted");
        assert_eq!(redacted.as_redacted_thinking().unwrap().data, "redacted");
    }

    #[test]
    fn message_content_filters_for_audience() {
        let assistant_text = MessageContent::Text(
            RawTextContent {
                text: "assistant only".to_string(),
                meta: None,
            }
            .with_audience(vec![Role::Assistant]),
        );
        assert!(assistant_text
            .filter_for_audience(Role::Assistant)
            .is_some());
        assert!(assistant_text.filter_for_audience(Role::User).is_none());

        let thinking = MessageContent::thinking("provider reasoning", "sig");
        assert!(thinking.filter_for_audience(Role::Assistant).is_some());
        assert!(thinking.filter_for_audience(Role::User).is_none());

        let response = MessageContent::tool_response(
            "tool_1",
            Ok(CallToolResult::success(vec![
                Content::text("assistant visible").with_audience(vec![Role::Assistant]),
                Content::text("user visible").with_audience(vec![Role::User]),
            ])),
        );
        let filtered = response.filter_for_audience(Role::Assistant).unwrap();
        let text = filtered.as_tool_response_text().unwrap();
        assert_eq!(text, "assistant visible");
    }

    #[test]
    fn message_content_converts_rmcp_content() {
        let text = MessageContent::from(Content::text("hello"));
        assert_eq!(text.as_text(), Some("hello"));

        let resource = rmcp::model::ResourceContents::TextResourceContents {
            uri: "file:///test.txt".to_string(),
            mime_type: Some("text/plain".to_string()),
            text: "resource text".to_string(),
            meta: None,
        };
        assert_eq!(extract_text_from_resource(&resource), "resource text");
    }

    #[test]
    fn extract_text_from_resource_handles_binary_blob() {
        let blob = base64::engine::general_purpose::STANDARD.encode([0xFF, 0xFE]);
        let resource = rmcp::model::ResourceContents::BlobResourceContents {
            uri: "file:///image.png".to_string(),
            mime_type: Some("image/png".to_string()),
            blob,
            meta: None,
        };

        assert_eq!(
            extract_text_from_resource(&resource),
            "[Binary content (image/png) - 2 bytes]"
        );
    }

    #[test]
    fn action_required_round_trips_tool_confirmation_payload() {
        let mut arguments = JsonObject::new();
        arguments.insert("path".to_string(), serde_json::json!("/tmp/example.txt"));

        let action_required = ActionRequired {
            data: ActionRequiredData::ToolConfirmation {
                id: "call_123".to_string(),
                tool_name: "developer__shell".to_string(),
                arguments,
                prompt: Some("Allow this tool call?".to_string()),
            },
        };

        let value = serde_json::to_value(&action_required).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "data": {
                    "actionType": "toolConfirmation",
                    "id": "call_123",
                    "toolName": "developer__shell",
                    "arguments": {"path": "/tmp/example.txt"},
                    "prompt": "Allow this tool call?"
                }
            })
        );

        assert_eq!(
            serde_json::from_value::<ActionRequired>(value).unwrap(),
            action_required
        );
    }

    #[test]
    fn openai_responses_model_matches_o_and_gpt5_families() {
        for model in [
            "o3",
            "o3-mini",
            "o4-mini",
            "gpt-5",
            "gpt-5-pro",
            "gpt-5.4",
            "gpt-5.4-mini",
            "gpt-5-4",
            "gpt-5-2-pro",
            "databricks-gpt-5.4",
            "goose-gpt-5.4-high",
            "headless-goose-o3-mini",
        ] {
            assert!(is_openai_responses_model(model), "{model} should match");
        }
    }

    #[test]
    fn openai_responses_model_rejects_other_families() {
        for model in [
            "gpt-4o",
            "claude-sonnet-4",
            "databricks-claude-sonnet-4",
            "llama-3-70b",
        ] {
            assert!(
                !is_openai_responses_model(model),
                "{model} should not match"
            );
        }
    }

    #[test]
    fn extract_reasoning_effort_for_responses_models() {
        for (model, expected_name, expected_effort) in [
            ("o3-none", "o3", Some("none")),
            ("o3-xhigh", "o3", Some("xhigh")),
            ("gpt-5-low", "gpt-5", Some("low")),
            ("gpt-5.4", "gpt-5.4", None),
            (
                "databricks-gpt-5.4-high",
                "databricks-gpt-5.4",
                Some("high"),
            ),
            ("databricks-o3-low", "databricks-o3", Some("low")),
            ("goose-gpt-5-high", "goose-gpt-5", Some("high")),
            ("gpt-4o", "gpt-4o", None),
        ] {
            let (name, effort) = extract_reasoning_effort(model);
            assert_eq!(name, expected_name, "unexpected base model for {model}");
            assert_eq!(
                effort.as_deref(),
                expected_effort,
                "unexpected effort for {model}"
            );
        }
    }

    #[test]
    fn openai_reasoning_effort_for_thinking_uses_supported_effort() {
        assert_eq!(
            openai_reasoning_effort_for_thinking("gpt-5-pro", ThinkingEffort::Max),
            Some("high".to_string())
        );
        assert_eq!(
            openai_reasoning_effort_for_thinking("gpt-5.4", ThinkingEffort::Max),
            Some("xhigh".to_string())
        );
        assert_eq!(
            openai_reasoning_effort_for_thinking("o3", ThinkingEffort::Off),
            Some("none".to_string())
        );
    }
}
