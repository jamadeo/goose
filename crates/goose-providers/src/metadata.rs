use goose_types::{ConfigKey, ModelInfo};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Metadata about a provider's configuration requirements and capabilities
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProviderMetadata {
    /// The unique identifier for this provider
    pub name: String,
    /// Display name for the provider in UIs
    pub display_name: String,
    /// Description of the provider's capabilities
    pub description: String,
    /// The default/recommended model for this provider
    pub default_model: String,
    /// A list of currently known models with their capabilities
    pub known_models: Vec<ModelInfo>,
    /// Link to the docs where models can be found
    pub model_doc_link: String,
    /// Required configuration keys
    pub config_keys: Vec<ConfigKey>,
    /// step-by-step instructions for set up providers eg: api key
    #[serde(default)]
    pub setup_steps: Vec<String>,
    /// Hint shown in the model picker when this provider manages its own model selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_selection_hint: Option<String>,
}

impl ProviderMetadata {
    pub fn new(
        name: &str,
        display_name: &str,
        description: &str,
        default_model: &str,
        model_names: Vec<&str>,
        model_doc_link: &str,
        config_keys: Vec<ConfigKey>,
    ) -> Self {
        Self {
            name: name.to_string(),
            display_name: display_name.to_string(),
            description: description.to_string(),
            default_model: default_model.to_string(),
            known_models: model_names
                .iter()
                .map(|&model_name| crate::models::model_info_for_provider_model(name, model_name))
                .collect(),
            model_doc_link: model_doc_link.to_string(),
            config_keys,
            setup_steps: vec![],
            model_selection_hint: None,
        }
    }

    pub fn with_models(
        name: &str,
        display_name: &str,
        description: &str,
        default_model: &str,
        models: Vec<ModelInfo>,
        model_doc_link: &str,
        config_keys: Vec<ConfigKey>,
    ) -> Self {
        Self {
            name: name.to_string(),
            display_name: display_name.to_string(),
            description: description.to_string(),
            default_model: default_model.to_string(),
            known_models: models,
            model_doc_link: model_doc_link.to_string(),
            config_keys,
            setup_steps: vec![],
            model_selection_hint: None,
        }
    }

    pub fn empty() -> Self {
        Self {
            name: "".to_string(),
            display_name: "".to_string(),
            description: "".to_string(),
            default_model: "".to_string(),
            known_models: vec![],
            model_doc_link: "".to_string(),
            config_keys: vec![],
            setup_steps: vec![],
            model_selection_hint: None,
        }
    }

    pub fn with_setup_steps(mut self, steps: Vec<&str>) -> Self {
        self.setup_steps = steps.into_iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_model_selection_hint(mut self, hint: &str) -> Self {
        self.model_selection_hint = Some(hint.to_string());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_metadata_new_applies_canonical_context_limits() {
        let metadata = ProviderMetadata::new(
            "test",
            "Test Provider",
            "Test Description",
            "gpt-4o",
            vec!["gpt-4o", "claude-sonnet-4-20250514", "unknown-model"],
            "https://example.com",
            vec![],
        );

        let model_info: std::collections::HashMap<String, usize> = metadata
            .known_models
            .into_iter()
            .map(|model| (model.name, model.context_limit))
            .collect();

        assert_eq!(*model_info.get("gpt-4o").unwrap(), 128_000);
        assert_eq!(
            *model_info.get("claude-sonnet-4-20250514").unwrap(),
            200_000
        );
        assert_eq!(*model_info.get("unknown-model").unwrap(), 128_000);
    }
}
