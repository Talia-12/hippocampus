use chrono::{DateTime, Utc};
use clap::Subcommand;
use hippocampus::dto::{GetQueryDto, SuspendedFilter};

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
        /// Only cards with next review before this datetime (RFC 3339)
        #[clap(long)]
        next_review_before: Option<DateTime<Utc>>,
        /// Suspended filter: Include, Exclude, or Only
        #[clap(long, default_value = "Exclude")]
        suspended_filter: String,
        /// Only cards suspended after this datetime (RFC 3339)
        #[clap(long)]
        suspended_after: Option<DateTime<Utc>>,
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
        #[clap(value_parser = clap::builder::BoolishValueParser::new())]
        suspend: bool,
    },
    /// Show all possible next review times for a card
    NextReviews {
        /// The card ID
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

/// Executes a card command
pub async fn execute(
    client: &HippocampusClient,
    cmd: CardCommands,
    config: &OutputConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CardCommands::List {
            item_type_id,
            next_review_before,
            suspended_filter,
            suspended_after,
        } => {
            let query = GetQueryDto {
                item_type_id,
                next_review_before,
                suspended_filter: parse_suspended_filter(&suspended_filter),
                suspended_after,
                ..Default::default()
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
    }
    Ok(())
}
