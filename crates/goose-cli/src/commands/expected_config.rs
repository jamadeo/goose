// This is a temporary file to simulate some configuration data from the backend

use crate::profile::provider_helper::PROVIDER_OPEN_AI;

pub struct RecommendedModels {
    pub processor: &'static str,
    pub accelerator: &'static str,
}
pub fn get_recommended_models(provider_name: &str) -> RecommendedModels {
    if provider_name == PROVIDER_OPEN_AI {
        RecommendedModels {
            processor: "gpt-4o",
            accelerator: "gpt-4o-mini",
        }
    } else {
        RecommendedModels {
            processor: "claude-3-5-sonnet-2",
            accelerator: "claude-3-5-sonnet-2",
        }
    }
}
