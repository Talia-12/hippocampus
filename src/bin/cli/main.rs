mod client;
mod output;

use clap::{Parser, Subcommand};
use client::HippocampusClient;
use hippocampus::config;
use output::OutputFormat;
use std::process;

/// CLI for the Hippocampus spaced repetition system
#[derive(Parser, Debug)]
#[clap(name = "hippocampus-cli", about = "CLI for the Hippocampus SRS")]
struct Cli {
    /// Server URL to connect to
    #[clap(
        long,
        env = "HIPPOCAMPUS_URL",
        global = true
    )]
    server_url: Option<String>,

    /// Output format
    #[clap(long, value_enum, default_value_t = OutputFormat::Human, global = true)]
    format: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Manage item types
    #[command(subcommand)]
    ItemType(ItemTypeCommands),
}

#[derive(Subcommand, Debug)]
enum ItemTypeCommands {
    /// List all item types
    List,
}

/// Resolves the server URL from CLI args, config file, or defaults
///
/// Precedence: CLI flag / env var > config file > default (port based on debug/release)
fn resolve_server_url(cli_url: Option<String>) -> String {
    if let Some(url) = cli_url {
        return url;
    }

    // Try reading from config file
    let config_dir = config::get_config_dir_path();
    if let Some(ref dir) = config_dir {
        let config_path = dir.join("config.toml");
        if let Ok(update) = config::config_from_file(Some(config_path)) {
            if let Some(url) = update.server_url {
                return url;
            }
        }
    }

    // Default: port 3001 in debug builds, 3000 in release
    let port = if cfg!(debug_assertions) { 3001 } else { 3000 };
    format!("http://localhost:{}", port)
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let server_url = resolve_server_url(cli.server_url);
    let client = HippocampusClient::new(server_url);

    let result = match cli.command {
        Commands::ItemType(cmd) => match cmd {
            ItemTypeCommands::List => run_list_item_types(&client, cli.format).await,
        },
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

/// Runs the item-type list command
async fn run_list_item_types(
    client: &HippocampusClient,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let item_types = client.list_item_types().await?;
    output::print_item_types(&item_types, format);
    Ok(())
}
