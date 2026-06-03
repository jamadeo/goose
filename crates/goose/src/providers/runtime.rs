use anyhow::Result;
use goose_providers::runtime::ProviderRuntime;
use serde::de::DeserializeOwned;

pub struct GooseProviderRuntime;

impl ProviderRuntime for GooseProviderRuntime {
    fn get_secret(&self, key: &str) -> Result<String> {
        Ok(crate::config::Config::global().get_secret(key)?)
    }

    fn get_param<T>(&self, key: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        Ok(crate::config::Config::global().get_param(key)?)
    }
}
