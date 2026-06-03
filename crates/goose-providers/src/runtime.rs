use anyhow::Result;
use serde::de::DeserializeOwned;

pub trait ProviderRuntime {
    fn get_secret(&self, key: &str) -> Result<String>;

    fn get_param<T>(&self, key: &str) -> Result<T>
    where
        T: DeserializeOwned;
}
