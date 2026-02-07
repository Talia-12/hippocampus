use clap::Subcommand;

use crate::client::HippocampusClient;
use crate::output::{self, OutputFormat};

/// Item management commands
#[derive(Subcommand, Debug)]
pub enum ItemCommands {
    /// List all items, optionally filtered by item type
    List {
        /// Filter by item type ID
        #[clap(long)]
        item_type_id: Option<String>,
    },
    /// Create a new item
    Create {
        /// The item type ID
        #[clap(long)]
        item_type_id: String,
        /// The item title
        #[clap(long)]
        title: String,
        /// JSON data for the item (default: {})
        #[clap(long, default_value = "{}")]
        data: String,
        /// Priority between 0.0 and 1.0 (default: 0.5)
        #[clap(long, default_value_t = 0.5)]
        priority: f32,
    },
    /// Get a specific item by ID
    Get {
        /// The item ID
        id: String,
    },
    /// Update an existing item
    Update {
        /// The item ID
        id: String,
        /// New title
        #[clap(long)]
        title: Option<String>,
        /// New JSON data
        #[clap(long)]
        data: Option<String>,
    },
    /// Delete an item
    Delete {
        /// The item ID
        id: String,
    },
}

/// Executes an item command
pub async fn execute(
    client: &HippocampusClient,
    cmd: ItemCommands,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ItemCommands::List { item_type_id } => {
            let items = client.list_items(item_type_id.as_deref()).await?;
            output::print_items(&items, format);
        }
        ItemCommands::Create {
            item_type_id,
            title,
            data,
            priority,
        } => {
            let item_data: serde_json::Value = serde_json::from_str(&data)?;
            let item = client
                .create_item(item_type_id, title, item_data, priority)
                .await?;
            output::print_item(&item, format);
        }
        ItemCommands::Get { id } => {
            let item = client.get_item(&id).await?;
            match item {
                Some(item) => output::print_item(&item, format),
                None => {
                    eprintln!("Item not found: {}", id);
                    std::process::exit(1);
                }
            }
        }
        ItemCommands::Update { id, title, data } => {
            let item_data = data
                .map(|d| serde_json::from_str(&d))
                .transpose()?;
            let item = client.update_item(&id, title, item_data).await?;
            output::print_item(&item, format);
        }
        ItemCommands::Delete { id } => {
            client.delete_item(&id).await?;
            output::print_success(&format!("Deleted item {}", id), format);
        }
    }
    Ok(())
}
