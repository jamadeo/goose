use crate::canonical::{map_to_canonical_model, CanonicalModel, CanonicalModelRegistry, Modality};
use crate::errors::ProviderError;
use goose_types::{extract_reasoning_effort, ModelConfig, ModelInfo};

fn canonical_model_from_registry(
    provider_name: &str,
    model_name: &str,
    registry: &CanonicalModelRegistry,
) -> Option<CanonicalModel> {
    let canonical_id =
        map_to_canonical_model(provider_name, model_name, registry).or_else(|| {
            let (base_model, _effort) = extract_reasoning_effort(model_name);
            if base_model != model_name {
                map_to_canonical_model(provider_name, &base_model, registry)
            } else {
                None
            }
        })?;

    let (canonical_provider, canonical_model) = canonical_id.split_once('/')?;
    registry.get(canonical_provider, canonical_model).cloned()
}

pub fn canonical_model_for_provider_model(
    provider_name: &str,
    model_name: &str,
) -> Option<CanonicalModel> {
    let registry = CanonicalModelRegistry::bundled().ok()?;
    canonical_model_from_registry(provider_name, model_name, registry)
}

pub fn model_config_with_canonical_limits(
    mut model_config: ModelConfig,
    provider_name: &str,
) -> ModelConfig {
    let Some(canonical) =
        canonical_model_for_provider_model(provider_name, &model_config.model_name)
    else {
        return model_config;
    };

    if model_config.context_limit.is_none() {
        model_config.context_limit = Some(canonical.limit.context);
    }
    if model_config.max_tokens.is_none() {
        model_config.max_tokens = canonical
            .limit
            .output
            .filter(|&output| output < canonical.limit.context)
            .map(|output| output as i32);
    }
    if model_config.reasoning.is_none() {
        model_config.reasoning = canonical.reasoning;
    }

    model_config
}

pub fn model_info_for_config(provider_name: &str, model_config: &ModelConfig) -> ModelInfo {
    let canonical = canonical_model_for_provider_model(provider_name, &model_config.model_name);
    let reasoning = canonical
        .as_ref()
        .and_then(|model| model.reasoning)
        .unwrap_or_else(|| model_config.is_reasoning_model());

    ModelInfo {
        name: model_config.model_name.clone(),
        resolved_model: None,
        context_limit: model_config.context_limit(),
        input_token_cost: None,
        output_token_cost: None,
        currency: None,
        supports_cache_control: None,
        reasoning,
    }
}

pub fn model_info_for_provider_model(provider_name: &str, model_name: &str) -> ModelInfo {
    let model_config = model_config_with_canonical_limits(
        ModelConfig {
            model_name: model_name.to_string(),
            ..Default::default()
        },
        provider_name,
    );
    model_info_for_config(provider_name, &model_config)
}

pub fn canonical_model_id(
    provider_name: &str,
    provider_model: &str,
) -> Result<Option<String>, ProviderError> {
    let registry = CanonicalModelRegistry::bundled().map_err(|e| {
        ProviderError::ExecutionError(format!("Failed to load canonical registry: {}", e))
    })?;

    Ok(map_to_canonical_model(
        provider_name,
        provider_model,
        registry,
    ))
}

pub fn recommended_models(
    provider_name: &str,
    all_models: Vec<String>,
    toolshim: bool,
    skip_canonical_filtering: bool,
) -> Result<Vec<String>, ProviderError> {
    if skip_canonical_filtering {
        return Ok(all_models);
    }

    let registry = CanonicalModelRegistry::bundled().map_err(|e| {
        ProviderError::ExecutionError(format!("Failed to load canonical registry: {}", e))
    })?;

    let mut models_with_dates: Vec<(String, Option<String>)> = all_models
        .iter()
        .filter_map(|model| {
            let canonical = canonical_model_from_registry(provider_name, model, registry)?;

            if !canonical.modalities.input.contains(&Modality::Text) {
                return None;
            }

            if !canonical.tool_call && !toolshim {
                return None;
            }

            Some((model.clone(), canonical.release_date))
        })
        .collect();

    models_with_dates.sort_by(|a, b| match (&a.1, &b.1) {
        (Some(date_a), Some(date_b)) => date_b.cmp(date_a),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.0.cmp(&b.0),
    });

    let recommended: Vec<String> = models_with_dates
        .into_iter()
        .map(|(name, _)| name)
        .collect();

    if recommended.is_empty() {
        Ok(all_models)
    } else {
        Ok(recommended)
    }
}

pub fn recommended_model_info(
    provider_name: &str,
    all_models: Vec<String>,
    toolshim: bool,
    skip_canonical_filtering: bool,
) -> Result<Vec<ModelInfo>, ProviderError> {
    Ok(recommended_models(
        provider_name,
        all_models,
        toolshim,
        skip_canonical_filtering,
    )?
    .iter()
    .map(|model_name| model_info_for_provider_model(provider_name, model_name))
    .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use goose_types::DEFAULT_CONTEXT_LIMIT;

    #[test]
    fn model_config_uses_canonical_limits() {
        let config = model_config_with_canonical_limits(
            ModelConfig {
                model_name: "gpt-4o".to_string(),
                ..Default::default()
            },
            "openai",
        );

        assert_eq!(config.context_limit, Some(128_000));
        assert_eq!(config.max_tokens, Some(16_384));
        assert_eq!(config.reasoning, Some(false));
    }

    #[test]
    fn model_config_preserves_explicit_values() {
        let config = model_config_with_canonical_limits(
            ModelConfig {
                model_name: "gpt-4o".to_string(),
                context_limit: Some(42),
                max_tokens: Some(7),
                reasoning: Some(true),
                ..Default::default()
            },
            "openai",
        );

        assert_eq!(config.context_limit, Some(42));
        assert_eq!(config.max_tokens, Some(7));
        assert_eq!(config.reasoning, Some(true));
    }

    #[test]
    fn model_info_falls_back_for_unknown_models() {
        let info = model_info_for_provider_model("unknown-provider", "custom-model");

        assert_eq!(info.name, "custom-model");
        assert_eq!(info.context_limit, DEFAULT_CONTEXT_LIMIT);
        assert!(!info.reasoning);
    }

    #[test]
    fn recommended_models_filters_and_sorts_canonical_text_tool_models() {
        let models = recommended_models(
            "openai",
            vec![
                "gpt-4o".to_string(),
                "gpt-4o-mini".to_string(),
                "unknown-model".to_string(),
            ],
            false,
            false,
        )
        .unwrap();

        assert_eq!(models, vec!["gpt-4o-mini", "gpt-4o"]);
    }

    #[test]
    fn recommended_models_falls_back_when_filter_removes_everything() {
        let models =
            recommended_models("openai", vec!["unknown-model".to_string()], false, false).unwrap();

        assert_eq!(models, vec!["unknown-model"]);
    }

    #[test]
    fn recommended_models_can_skip_canonical_filtering() {
        let models =
            recommended_models("openai", vec!["unknown-model".to_string()], false, true).unwrap();

        assert_eq!(models, vec!["unknown-model"]);
    }
}
