use chrono::{DateTime, Utc};
use clap::Subcommand;
use hippocampus::dto::{GetQueryDto, SuspendedFilter};

use crate::client::HippocampusClient;
use crate::output::{self, OutputConfig};

/// Item management commands
#[derive(Subcommand, Debug)]
pub enum ItemCommands {
    /// List all items, with optional filtering
    List {
        /// Filter by item type ID
        #[clap(long)]
        item_type_id: Option<String>,
        /// Filter by tag IDs
        #[clap(long)]
        tag_ids: Vec<String>,
        /// Only cards with next review before this datetime (RFC 3339)
        #[clap(long)]
        next_review_before: Option<DateTime<Utc>>,
        /// Only cards with last review after this datetime (RFC 3339)
        #[clap(long)]
        last_review_after: Option<DateTime<Utc>>,
        /// Suspended filter: Include, Exclude, or Only
        #[clap(long, default_value = "exclude")]
        suspended_filter: String,
        /// Only cards suspended after this datetime (RFC 3339)
        #[clap(long)]
        suspended_after: Option<DateTime<Utc>>,
        /// Only cards suspended before this datetime (RFC 3339)
        #[clap(long)]
        suspended_before: Option<DateTime<Utc>>,
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

/// Parses a suspended filter string into the enum
fn parse_suspended_filter(s: &str) -> SuspendedFilter {
    match s.to_lowercase().as_str() {
        "include" => SuspendedFilter::Include,
        "only" => SuspendedFilter::Only,
        _ => SuspendedFilter::Exclude,
    }
}

/// Executes an item command
pub async fn execute(
    client: &HippocampusClient,
    cmd: ItemCommands,
    config: &OutputConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        ItemCommands::List {
            item_type_id,
            tag_ids,
            next_review_before,
            last_review_after,
            suspended_filter,
            suspended_after,
            suspended_before,
        } => {
            let query = GetQueryDto {
                item_type_id,
                tag_ids,
                next_review_before,
                last_review_after,
                suspended_filter: parse_suspended_filter(&suspended_filter),
                suspended_after,
                suspended_before,
                split_priority: None,
            };
            let items = client.list_items(&query).await?;
            output::print_items(&items, config);
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
            output::print_item(&item, config);
        }
        ItemCommands::Get { id } => {
            let item = client.get_item(&id).await?;
            match item {
                Some(item) => output::print_item(&item, config),
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
            output::print_item(&item, config);
        }
        ItemCommands::Delete { id } => {
            client.delete_item(&id).await?;
            output::print_success(&format!("Deleted item {}", id), config);
        }
    }
    Ok(())
}
