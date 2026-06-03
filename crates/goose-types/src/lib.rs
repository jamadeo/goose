use regex::{Regex, RegexBuilder};
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
