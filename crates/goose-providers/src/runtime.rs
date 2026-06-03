use anyhow::Result;
use serde::de::DeserializeOwned;
use std::collections::HashMap;

pub trait ProviderRuntime {
    fn get_secret(&self, key: &str) -> Result<String>;

    fn get_secrets(&self, primary: &str, maybe_secret: &[&str]) -> Result<HashMap<String, String>>;

    fn get_param<T>(&self, key: &str) -> Result<T>
    where
        T: DeserializeOwned;
}
