use anyhow::Result;
use bat::PrettyPrinter;
use clap::Parser;
use cliclack::{input, spinner};
use console::style;
use futures::StreamExt;
use goose::providers::factory::ProviderType;

use goose::agent::Agent;
use goose::developer::DeveloperSystem;
use goose::providers::configs::OpenAiProviderConfig;
use goose::providers::configs::{DatabricksProviderConfig, ProviderConfig};
use goose::providers::factory;
use goose::providers::types::message::Message;

mod commands {
    pub mod version;
}
#[derive(Parser)]
#[command(author, about, long_about = None)]
struct Cli {
    /// Provider option (openai or databricks)
    #[arg(short, long, default_value = "open-ai")]
    #[arg(value_enum)]
    provider: CliProviderVariant,

    /// OpenAI API Key (can also be set via OPENAI_API_KEY environment variable)
    #[arg(long)]
    api_key: Option<String>,

    /// Databricks host (can also be set via DATABRICKS_HOST environment variable)
    #[arg(long)]
    databricks_host: Option<String>,

    /// Databricks token (can also be set via DATABRICKS_TOKEN environment variable)
    #[arg(long)]
    databricks_token: Option<String>,

    /// Model to use
    #[arg(short, long, default_value = "gpt-4o")]
    model: String,

    #[arg(short = 'v', long = "version")]
    version: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum CliProviderVariant {
    OpenAi,
    Databricks,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.version {
        println!("{}", commands::version::get_version());
        return Ok(());
    }
    let provider_type = match cli.provider {
        CliProviderVariant::OpenAi => ProviderType::OpenAi,
        CliProviderVariant::Databricks => ProviderType::Databricks,
    };

    println!(
        "Example goose CLI {}",
        style("- type \"exit\" to end the session").dim()
    );
    println!("\n");

    let system = Box::new(DeveloperSystem::new());
    let provider = factory::get_provider(provider_type, create_provider_config(&cli)).unwrap();
    let mut agent = Agent::new(provider, cli.model.clone());
    agent.add_system(system);
    println!("Connected the developer system");

    let mut messages = Vec::new();

    loop {
        let message_text: String = input("Message:").placeholder("").multiline().interact()?;
        if message_text.trim().eq_ignore_ascii_case("exit") {
            break;
        }
        messages.push(Message::user(&message_text).unwrap());

        let spin = spinner();
        spin.start("awaiting reply");

        // Process the stream of messages
        let mut stream = agent.reply(&messages);
        while let Some(response) = stream.next().await {
            match response {
                Ok(message) => {
                    messages.push(message.clone());
                    for content in &message.content {
                        render(&content.summary()).await;
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    break;
                }
            }
        }
        spin.stop("");

        println!("\n");
    }
    Ok(())
}

async fn render(content: &str) {
    PrettyPrinter::new()
        .input_from_bytes(content.as_bytes())
        .language("markdown")
        .print()
        .unwrap();
}

fn create_provider_config(cli: &Cli) -> ProviderConfig {
    match cli.provider {
        CliProviderVariant::OpenAi => ProviderConfig::OpenAi(OpenAiProviderConfig {
            host: "https://api.openai.com".to_string(),
            api_key: cli.api_key.clone().unwrap_or_else(|| {
                std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set")
            }),
        }),
        CliProviderVariant::Databricks => ProviderConfig::Databricks(DatabricksProviderConfig {
            host: cli.databricks_host.clone().unwrap_or_else(|| {
                std::env::var("DATABRICKS_HOST").expect("DATABRICKS_HOST must be set")
            }),
            token: cli.databricks_token.clone().unwrap_or_else(|| {
                std::env::var("DATABRICKS_TOKEN").expect("DATABRICKS_TOKEN must be set")
            }),
        }),
    }
}