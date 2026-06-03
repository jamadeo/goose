use super::api_client::tls_config_from_goose_config;
use super::base::{ProviderDef, ProviderMetadata};
use super::openai_compatible::OpenAiCompatibleProvider;
use super::runtime::GooseProviderRuntime;
use anyhow::Result;
use futures::future::BoxFuture;
use goose_types::ModelConfig;

pub struct AvianProvider;

impl ProviderDef for AvianProvider {
    type Provider = OpenAiCompatibleProvider;

    fn metadata() -> ProviderMetadata {
        ProviderMetadata::new(
            goose_providers::avian::AVIAN_PROVIDER_NAME,
            goose_providers::avian::AVIAN_DISPLAY_NAME,
            goose_providers::avian::AVIAN_DESCRIPTION,
            goose_providers::avian::AVIAN_DEFAULT_MODEL,
            goose_providers::avian::AVIAN_KNOWN_MODELS.to_vec(),
            goose_providers::avian::AVIAN_DOC_URL,
            goose_providers::avian::config_keys(),
        )
    }

    fn from_env(
        model: ModelConfig,
        _extensions: Vec<crate::config::ExtensionConfig>,
    ) -> BoxFuture<'static, Result<OpenAiCompatibleProvider>> {
        Box::pin(async move {
            let runtime = GooseProviderRuntime;
            let api_client =
                goose_providers::avian::api_client(&runtime, tls_config_from_goose_config()?)?;

            Ok(OpenAiCompatibleProvider::new(
                goose_providers::avian::AVIAN_PROVIDER_NAME.to_string(),
                api_client,
                model,
                String::new(),
            ))
        })
    }
}
