use clap::ValueEnum;
use hippocampus::models::ItemType;

/// Output format for CLI commands
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable output
    Human,
    /// JSON output
    Json,
    /// Waybar-compatible JSON output
    Waybar,
}

/// Prints a list of item types in the specified format
///
/// ### Arguments
///
/// * `item_types` - The item types to print
/// * `format` - The output format to use
pub fn print_item_types(item_types: &[ItemType], format: OutputFormat) {
    match format {
        OutputFormat::Human => {
            if item_types.is_empty() {
                println!("No item types found.");
                return;
            }
            for item_type in item_types {
                println!("{}\t{}", item_type.get_id(), item_type.get_name());
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(item_types).unwrap());
        }
        OutputFormat::Waybar => {
            println!("{}", serde_json::to_string(item_types).unwrap());
        }
    }
}
