use crate::profile::profile::AdditionalSystem;
use crate::profile::profile_handler::{load_profiles, save_profile};
use serde_json::Value;
use std::error::Error;

pub async fn fetch_system(url: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let full_url = format!("{}/fetch_name", url);
    match reqwest::get(full_url).await {
        Ok(response) => match response.json::<Value>().await {
            Ok(json) => {
                if let Some(name) = json.get("name").and_then(|n| n.as_str()) {
                    return Ok(Some(name.to_string()));
                } else {
                    println!("No 'name' field in the JSON response.");
                }
            }
            Err(err) => {
                println!("Failed to parse JSON: {}", err);
            }
        },
        Err(err) => {
            println!("Failed to fetch URL: {}", err);
        }
    }
    Ok(None)
}

pub async fn add_system(url: String) -> Result<(), Box<dyn Error>> {
    let system_name = fetch_system(url.as_str()).await?;
    if system_name.is_none() {
        println!("System not found. Please enter a valid system location.");
        return Ok(());
    }
    let system_name = system_name.unwrap();
    match load_profiles() {
        Ok(mut profiles) => {
            if profiles.is_empty() {
                println!("No profiles found. Please create a profile first via goose configure");
                return Ok(());
            }
            for (profile_name, profile) in profiles.iter_mut() {
                if profile.additional_systems.iter().any(|s| s.location == url) {
                    continue;
                }
                profile.additional_systems.push(AdditionalSystem {
                    name: system_name.to_string(),
                    location: url.to_string(),
                });
                save_profile(profile_name, profile.clone())?
            }
            println!(
                "System '{}' at '{}' added to all profiles",
                system_name, url
            );
            Ok(())
        }
        Err(err) => {
            println!("Failed to load profiles: {}", err);
            Ok(())
        }
    }
}

pub async fn remove_system(url: String) -> Result<(), Box<dyn Error>> {
    let mut removed = false;
    match load_profiles() {
        Ok(mut profiles) => {
            if profiles.is_empty() {
                return Ok(());
            }
            for (profile_name, profile) in profiles.iter_mut() {
                if let Some(pos) = profile
                    .additional_systems
                    .iter()
                    .position(|s| s.location == url)
                {
                    profile.additional_systems.remove(pos);
                    save_profile(profile_name, profile.clone())?;
                    removed = true;
                }
            }
            if removed {
                println!("System at '{}' has been removed from all profiles", url);
            } else {
                println!("System at '{}' not found in any profiles", url);
            }
            Ok(())
        }
        Err(err) => {
            println!("Failed to load profiles: {}", err);
            Ok(())
        }
    }
}
