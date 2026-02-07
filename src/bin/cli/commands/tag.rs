use clap::Subcommand;

use crate::client::HippocampusClient;
use crate::output::{self, OutputConfig};

/// Tag management commands
#[derive(Subcommand, Debug)]
pub enum TagCommands {
    /// List all tags
    List,
    /// Create a new tag
    Create {
        /// Name of the tag
        #[clap(long)]
        name: String,
        /// Whether the tag is visible
        #[clap(long)]
        visible: bool,
    },
    /// Add a tag to an item
    Add {
        /// The item ID
        item_id: String,
        /// The tag ID
        tag_id: String,
    },
    /// Remove a tag from an item
    Remove {
        /// The item ID
        item_id: String,
        /// The tag ID
        tag_id: String,
    },
    /// List tags for a specific item
    ListForItem {
        /// The item ID
        item_id: String,
    },
}

/// Executes a tag command
pub async fn execute(
    client: &HippocampusClient,
    cmd: TagCommands,
    config: &OutputConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        TagCommands::List => {
            let tags = client.list_tags().await?;
            output::print_tags(&tags, config);
        }
        TagCommands::Create { name, visible } => {
            let tag = client.create_tag(name, visible).await?;
            output::print_tag(&tag, config);
        }
        TagCommands::Add { item_id, tag_id } => {
            client.add_tag_to_item(&item_id, &tag_id).await?;
            output::print_success(
                &format!("Added tag {} to item {}", tag_id, item_id),
                config,
            );
        }
        TagCommands::Remove { item_id, tag_id } => {
            client.remove_tag_from_item(&item_id, &tag_id).await?;
            output::print_success(
                &format!("Removed tag {} from item {}", tag_id, item_id),
                config,
            );
        }
        TagCommands::ListForItem { item_id } => {
            let tags = client.list_tags_for_item(&item_id).await?;
            output::print_tags(&tags, config);
        }
    }
    Ok(())
}
