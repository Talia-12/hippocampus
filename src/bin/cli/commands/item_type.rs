use clap::Subcommand;

use crate::client::HippocampusClient;
use crate::output::{self, OutputConfig};

/// Item type management commands
#[derive(Subcommand, Debug)]
pub enum ItemTypeCommands {
    /// List all item types
    List,
    /// Create a new item type
    Create {
        /// Name of the item type
        #[clap(long)]
        name: String,
        /// Review function to use for scheduling (default: "fsrs")
        #[clap(long, default_value = "fsrs")]
        review_function: String,
    },
    /// Get a specific item type by ID
    Get {
        /// The item type ID
        id: String,
    },
    /// Update an item type's review function
    Update {
        /// The item type ID
        id: String,
        /// The new review function
        #[clap(long)]
        review_function: String,
    },
}

/// Executes an item type command
pub async fn execute(
    client: &HippocampusClient,
    cmd: ItemTypeCommands,
    config: &OutputConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ItemTypeCommands::List => {
            let item_types = client.list_item_types().await?;
            output::print_item_types(&item_types, config);
        }
        ItemTypeCommands::Create { name, review_function } => {
            let item_type = client.create_item_type(name, Some(review_function)).await?;
            output::print_item_type(&item_type, config);
        }
        ItemTypeCommands::Get { id } => {
            let item_type = client.get_item_type(&id).await?;
            output::print_item_type(&item_type, config);
        }
        ItemTypeCommands::Update { id, review_function } => {
            let item_type = client.update_item_type(&id, review_function).await?;
            output::print_item_type(&item_type, config);
        }
    }
    Ok(())
}
