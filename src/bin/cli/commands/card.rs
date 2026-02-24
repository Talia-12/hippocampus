use chrono::{DateTime, Utc};
use clap::Subcommand;
use hippocampus::dto::{GetQueryDto, SortPositionAction, SuspendedFilter};

use crate::client::HippocampusClient;
use crate::output::{self, OutputConfig};

/// Card management commands
#[derive(Subcommand, Debug)]
pub enum CardCommands {
    /// List cards with optional filters
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
        /// Whether base priority and priority offset should be returned as separate fields
        #[clap(long)]
        split_priority: bool,
    },
    /// Get a specific card by ID
    Get {
        /// The card ID
        id: String,
    },
    /// Update a card's priority
    Priority {
        /// The card ID
        id: String,
        /// The new priority value (0.0 to 1.0)
        value: f32,
    },
    /// Suspend or unsuspend a card
    Suspend {
        /// The card ID
        id: String,
        /// true to suspend, false to unsuspend
        #[clap(num_args = 1, required = true, value_parser = clap::builder::BoolishValueParser::new())]
        suspend: bool,
    },
    /// Show all possible next review times for a card
    NextReviews {
        /// The card ID
        id: String,
    },
    /// Reorder a card to the top of the queue
    ReorderToTop {
        /// The card ID
        id: String,
    },
    /// Reorder a card to before another card
    ReorderBefore {
        /// The card ID to move
        id_to_move: String,
        /// The card ID to move before
        target_id: String,
    },
    /// Reorder a card to after another card
    ReorderAfter {
        /// The card ID to move
        id_to_move: String,
        /// The card ID to move after
        target_id: String,
    },
    /// Clear user reordering, for all cards matching the query
    ClearOrdering {
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
}

/// Parses a suspended filter string into the enum
fn parse_suspended_filter(s: &str) -> SuspendedFilter {
    match s.to_lowercase().as_str() {
        "include" => SuspendedFilter::Include,
        "only" => SuspendedFilter::Only,
        _ => SuspendedFilter::Exclude,
    }
}

/// Executes a card command
pub async fn execute(
    client: &HippocampusClient,
    cmd: CardCommands,
    config: &OutputConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CardCommands::List {
            item_type_id,
            tag_ids,
            next_review_before,
            last_review_after,
            suspended_filter,
            suspended_after,
            suspended_before,
            split_priority,
        } => {
            let query = GetQueryDto {
                item_type_id,
                tag_ids,
                next_review_before,
                last_review_after,
                suspended_filter: parse_suspended_filter(&suspended_filter),
                suspended_after,
                suspended_before,
                split_priority: if split_priority { Some(true) } else { None },
            };
            let cards = client.list_cards(&query).await?;
            output::print_cards(&cards, config);
        }
        CardCommands::Get { id } => {
            let card = client.get_card(&id).await?;
            match card {
                Some(card) => output::print_card(&card, config),
                None => {
                    eprintln!("Card not found: {}", id);
                    std::process::exit(1);
                }
            }
        }
        CardCommands::Priority { id, value } => {
            let card = client.update_card_priority(&id, value).await?;
            output::print_card(&card, config);
        }
        CardCommands::Suspend { id, suspend } => {
            client.suspend_card(&id, suspend).await?;
            let action = if suspend { "Suspended" } else { "Unsuspended" };
            output::print_success(&format!("{} card {}", action, id), config);
        }
        CardCommands::NextReviews { id } => {
            let next_reviews = client.get_next_reviews(&id).await?;
            output::print_next_reviews(&next_reviews, config);
        }
        CardCommands::ReorderToTop { id } => {
            let card = client.set_sort_position(&id, &SortPositionAction::Top).await?;
            output::print_card_json(&card, config);
        }
        CardCommands::ReorderBefore { id_to_move, target_id } => {
            let card = client
                .set_sort_position(&id_to_move, &SortPositionAction::Before { card_id: target_id })
                .await?;
            output::print_card_json(&card, config);
        }
        CardCommands::ReorderAfter { id_to_move, target_id } => {
            let card = client
                .set_sort_position(&id_to_move, &SortPositionAction::After { card_id: target_id })
                .await?;
            output::print_card_json(&card, config);
        }
        CardCommands::ClearOrdering {
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
            client.clear_sort_positions(&query).await?;
            output::print_success("Cleared card ordering", config);
        }
    }
    Ok(())
}
