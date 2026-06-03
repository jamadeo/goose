use crate::api_client::{ApiClient, AuthMethod, TlsConfig, DEFAULT_PROVIDER_TIMEOUT_SECS};
use crate::runtime::ProviderRuntime;
use anyhow::Result;
use goose_types::ConfigKey;
use std::time::Duration;

pub const AVIAN_PROVIDER_NAME: &str = "avian";
pub const AVIAN_DISPLAY_NAME: &str = "Avian";
pub const AVIAN_DESCRIPTION: &str =
    "Cost-effective inference API with DeepSeek, Kimi, GLM, and MiniMax models";
pub const AVIAN_API_HOST: &str = "https://api.avian.io/v1";
pub const AVIAN_DEFAULT_MODEL: &str = "deepseek/deepseek-v3.2";
pub const AVIAN_KNOWN_MODELS: &[&str] = &[
    "deepseek/deepseek-v3.2",
    "moonshotai/kimi-k2.5",
    "z-ai/glm-5",
    "minimax/minimax-m2.5",
];
pub const AVIAN_DOC_URL: &str = "https://avian.io/docs";

pub fn config_keys() -> Vec<ConfigKey> {
    vec![
        ConfigKey::new("AVIAN_API_KEY", true, true, None, true),
        ConfigKey::new("AVIAN_HOST", false, false, Some(AVIAN_API_HOST), false),
    ]
}

pub fn api_client<R: ProviderRuntime>(
    runtime: &R,
    tls_config: Option<TlsConfig>,
) -> Result<ApiClient> {
    let api_key = runtime.get_secret("AVIAN_API_KEY")?;
    let host = runtime
        .get_param("AVIAN_HOST")
        .unwrap_or_else(|_| AVIAN_API_HOST.to_string());

    ApiClient::with_timeout_and_tls_config(
        host,
        AuthMethod::BearerToken(api_key),
        Duration::from_secs(DEFAULT_PROVIDER_TIMEOUT_SECS),
        tls_config,
    )
}
