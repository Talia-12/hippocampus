mod client;
mod commands;
mod output;

use clap::{Parser, Subcommand};
use client::HippocampusClient;
use hippocampus::config;
use output::{OutputConfig, OutputFormat};
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

    /// Quiet mode: minimal output (just IDs or counts)
    #[clap(short, long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Manage item types
    #[command(subcommand)]
    ItemType(commands::item_type::ItemTypeCommands),
    /// Manage items
    #[command(subcommand)]
    Item(commands::item::ItemCommands),
    /// Manage cards
    #[command(subcommand)]
    Card(commands::card::CardCommands),
    /// Manage reviews
    #[command(subcommand)]
    Review(commands::review::ReviewCommands),
    /// Manage tags
    #[command(subcommand)]
    Tag(commands::tag::TagCommands),
    /// High-level todo workflow commands
    #[command(subcommand)]
    Todo(commands::todo::TodoCommands),
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

/// Formats an error for human-readable stderr output
fn format_error(err: &dyn std::error::Error) -> String {
    let err_string = err.to_string();

    // ClientError::Request wraps reqwest errors — check for connection issues
    if err_string.contains("error sending request")
        || err_string.contains("connection refused")
        || err_string.contains("Connection refused")
        || err_string.contains("tcp connect error")
    {
        return format!(
            "Could not connect to server. Is hippocampus running?\n  {}",
            err_string
        );
    }

    // ClientError::Server already formats as "Server error (STATUS): message"
    // so we can return it directly
    err_string
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let server_url = resolve_server_url(cli.server_url);
    let client = HippocampusClient::new(server_url);
    let output_config = OutputConfig {
        format: cli.format,
        quiet: cli.quiet,
    };

    let result = match cli.command {
        Commands::ItemType(cmd) => {
            commands::item_type::execute(&client, cmd, &output_config).await
        }
        Commands::Item(cmd) => commands::item::execute(&client, cmd, &output_config).await,
        Commands::Card(cmd) => commands::card::execute(&client, cmd, &output_config).await,
        Commands::Review(cmd) => commands::review::execute(&client, cmd, &output_config).await,
        Commands::Tag(cmd) => commands::tag::execute(&client, cmd, &output_config).await,
        Commands::Todo(cmd) => commands::todo::execute(&client, cmd, &output_config).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {}", format_error(e.as_ref()));
        process::exit(1);
    }
}
