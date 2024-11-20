use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile {
    pub provider: String,
    pub processor: String,
    pub accelerator: String,
    #[serde(default)]
    pub additional_systems: Vec<AdditionalSystem>,
}

#[derive(Serialize, Deserialize)]
pub struct Profiles {
    pub profile_items: std::collections::HashMap<String, Profile>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AdditionalSystem {
    pub name: String,
    pub location: String,
}
