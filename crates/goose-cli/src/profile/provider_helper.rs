use crate::inputs::inputs::get_env_value_or_input;
use goose::providers::configs::{DatabricksProviderConfig, OpenAiProviderConfig, ProviderConfig};
use goose::providers::factory::ProviderType;
use strum::IntoEnumIterator;

pub const PROVIDER_OPEN_AI: &str = "openai";
pub const PROVIDER_DATABRICKS: &str = "databricks";

pub fn select_provider_lists() -> Vec<(&'static str, String, &'static str)> {
    ProviderType::iter()
        .map(|provider| match provider {
            ProviderType::OpenAi => (
                PROVIDER_OPEN_AI,
                PROVIDER_OPEN_AI.to_string(),
                "Recommended",
            ),
            ProviderType::Databricks => (PROVIDER_DATABRICKS, PROVIDER_DATABRICKS.to_string(), ""),
        })
        .collect()
}

pub fn set_provider_config(provider_name: &str, processor: String) -> ProviderConfig {
    match provider_name.to_lowercase().as_str() {
        PROVIDER_OPEN_AI => ProviderConfig::OpenAi(OpenAiProviderConfig {
            host: "https://api.openai.com".to_string(),
            api_key: get_env_value_or_input(
                "OPENAI_API_KEY",
                "Please enter your OpenAI API key:",
                true,
            ),
            model: processor,
            temperature: None,
            max_tokens: None,
        }),
        PROVIDER_DATABRICKS => ProviderConfig::Databricks(DatabricksProviderConfig {
            host: get_env_value_or_input(
                "DATABRICKS_HOST",
                "Please enter your Databricks host:",
                false,
            ),
            token: get_env_value_or_input(
                "DATABRICKS_TOKEN",
                "Please enter your Databricks token:",
                true,
            ),
            model: processor,
            temperature: None,
            max_tokens: None,
        }),
        _ => panic!("Invalid provider name"),
    }
}
