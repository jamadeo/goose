use crate::errors::ProviderError;
use crate::models;
use crate::retry::RetryConfig;
use async_trait::async_trait;
use futures::Stream;
use goose_types::{ModelConfig, ModelInfo, ProviderUsage};
use std::pin::Pin;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionRouting {
    ActionRequired,
    Noop,
}

pub type MessageStream<Message> = Pin<
    Box<dyn Stream<Item = Result<(Option<Message>, Option<ProviderUsage>), ProviderError>> + Send>,
>;

#[async_trait]
pub trait Provider: Send + Sync {
    type Message: Send + Sync + 'static;
    type Tool: Send + Sync + 'static;
    type Conversation: Send + Sync + 'static;
    type PermissionConfirmation: Send + Sync + 'static;
    type Mode: Send + Sync + 'static;

    fn get_name(&self) -> &str;

    async fn stream(
        &self,
        model_config: &ModelConfig,
        session_id: &str,
        system: &str,
        messages: &[Self::Message],
        tools: &[Self::Tool],
    ) -> Result<MessageStream<Self::Message>, ProviderError>;

    fn get_model_config(&self) -> ModelConfig;

    fn retry_config(&self) -> RetryConfig {
        RetryConfig::default()
    }

    async fn fetch_supported_models(&self) -> Result<Vec<String>, ProviderError> {
        Ok(vec![])
    }

    async fn fetch_supported_model_info(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(self
            .fetch_supported_models()
            .await?
            .iter()
            .map(|model_name| models::model_info_for_provider_model(self.get_name(), model_name))
            .collect())
    }

    async fn fetch_model_info(&self, model_name: &str) -> Result<ModelInfo, ProviderError> {
        Ok(models::model_info_for_provider_model(
            self.get_name(),
            model_name,
        ))
    }

    fn skip_canonical_filtering(&self) -> bool {
        false
    }

    async fn fetch_recommended_models(&self) -> Result<Vec<String>, ProviderError> {
        models::recommended_models(
            self.get_name(),
            self.fetch_supported_models().await?,
            self.get_model_config().toolshim,
            self.skip_canonical_filtering(),
        )
    }

    async fn fetch_recommended_model_info(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(self
            .fetch_recommended_models()
            .await?
            .iter()
            .map(|model_name| models::model_info_for_provider_model(self.get_name(), model_name))
            .collect())
    }

    async fn map_to_canonical_model(
        &self,
        provider_model: &str,
    ) -> Result<Option<String>, ProviderError> {
        models::canonical_model_id(self.get_name(), provider_model)
    }

    fn supports_embeddings(&self) -> bool {
        false
    }

    fn manages_own_context(&self) -> bool {
        false
    }

    async fn supports_cache_control(&self) -> bool {
        false
    }

    async fn create_embeddings(
        &self,
        _session_id: &str,
        _texts: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        Err(ProviderError::ExecutionError(
            "This provider does not support embeddings".to_string(),
        ))
    }

    fn get_initial_user_messages(&self, _messages: &Self::Conversation) -> Vec<String> {
        Vec::new()
    }

    fn get_preprompt_context(&self, _messages: &Self::Conversation) -> String {
        String::new()
    }

    async fn generate_session_name(
        &self,
        _session_id: &str,
        _messages: &Self::Conversation,
    ) -> Result<String, ProviderError> {
        Err(ProviderError::NotImplemented(
            "generate_session_name not implemented for this provider".to_string(),
        ))
    }

    async fn configure_oauth(&self) -> Result<(), ProviderError> {
        Err(ProviderError::ExecutionError(
            "OAuth configuration not supported by this provider".to_string(),
        ))
    }

    async fn refresh_credentials(&self) -> Result<(), ProviderError> {
        Err(ProviderError::NotImplemented(
            "credential refresh not supported by this provider".to_string(),
        ))
    }

    async fn update_mode(&self, _session_id: &str, _mode: Self::Mode) -> Result<(), ProviderError> {
        Ok(())
    }

    fn permission_routing(&self) -> PermissionRouting {
        PermissionRouting::Noop
    }

    async fn handle_permission_confirmation(
        &self,
        _request_id: &str,
        _confirmation: &Self::PermissionConfirmation,
    ) -> bool {
        false
    }
}
