use chrono::{TimeZone, Utc};
use clap::Subcommand;
use hippocampus::dto::{GetQueryDto, SuspendedFilter};
use hippocampus::models::{Card, Item};

use crate::client::HippocampusClient;
use crate::output::{self, OutputFormat};

/// High-level todo workflow commands
#[derive(Subcommand, Debug)]
pub enum TodoCommands {
    /// Add a new todo item
    Add {
        /// The title of the todo
        title: String,
        /// Item type name or ID
        #[clap(long)]
        item_type: String,
        /// Tags to attach (by name or ID), can be specified multiple times
        #[clap(long)]
        tag: Vec<String>,
        /// JSON data for the item
        #[clap(long, default_value = "{}")]
        data: String,
    },
    /// List due todos (cards with next review before tomorrow midnight)
    Due {
        /// Filter by item type name or ID
        #[clap(long)]
        item_type: Option<String>,
        /// Filter by tag name or ID, can be specified multiple times
        #[clap(long)]
        tag: Vec<String>,
    },
    /// List recently completed todos (suspended today)
    Completed {
        /// Filter by item type name or ID
        #[clap(long)]
        item_type: Option<String>,
    },
    /// Mark a todo as complete (suspend the card)
    Complete {
        /// The card ID to complete
        card_id: String,
    },
    /// Mark a todo as incomplete (unsuspend the card)
    Uncomplete {
        /// The card ID to uncomplete
        card_id: String,
    },
    /// Record a review for a card
    Review {
        /// The card ID to review
        card_id: String,
        /// The rating (1-4)
        rating: i32,
    },
    /// Count due todos (useful for waybar)
    Count {
        /// Filter by item type name or ID
        #[clap(long)]
        item_type: Option<String>,
        /// Filter by tag name or ID, can be specified multiple times
        #[clap(long)]
        tag: Vec<String>,
    },
}

/// Resolves an item type name or ID to its ID
async fn resolve_item_type_id(
    client: &HippocampusClient,
    name_or_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let item_types = client.list_item_types().await?;
    // Try name match first (case-insensitive)
    for item_type in &item_types {
        if item_type.get_name().eq_ignore_ascii_case(name_or_id) {
            return Ok(item_type.get_id());
        }
    }
    // Fall back to ID match
    for item_type in &item_types {
        if item_type.get_id() == name_or_id {
            return Ok(item_type.get_id());
        }
    }
    Err(format!("Item type not found: {}", name_or_id).into())
}

/// Resolves a list of tag names or IDs to their IDs
async fn resolve_tag_ids(
    client: &HippocampusClient,
    names_or_ids: &[String],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    if names_or_ids.is_empty() {
        return Ok(Vec::new());
    }
    let all_tags = client.list_tags().await?;
    let mut ids = Vec::new();
    for name_or_id in names_or_ids {
        let found = all_tags.iter().find(|t| {
            t.get_name().eq_ignore_ascii_case(name_or_id) || t.get_id() == *name_or_id
        });
        match found {
            Some(tag) => ids.push(tag.get_id()),
            None => return Err(format!("Tag not found: {}", name_or_id).into()),
        }
    }
    Ok(ids)
}

/// Returns the start of tomorrow (midnight UTC)
fn tomorrow_midnight() -> chrono::DateTime<Utc> {
    let today = Utc::now().date_naive();
    let tomorrow = today.succ_opt().expect("date overflow");
    Utc.from_utc_datetime(&tomorrow.and_hms_opt(0, 0, 0).expect("invalid time"))
}

/// Returns the start of today (midnight UTC)
fn today_midnight() -> chrono::DateTime<Utc> {
    let today = Utc::now().date_naive();
    Utc.from_utc_datetime(&today.and_hms_opt(0, 0, 0).expect("invalid time"))
}

/// Fetches items for a list of cards, returning paired results
async fn fetch_cards_with_items(
    client: &HippocampusClient,
    cards: Vec<Card>,
) -> Result<Vec<(Card, Option<Item>)>, Box<dyn std::error::Error>> {
    let mut result = Vec::new();
    for card in cards {
        let item = client.get_item(&card.get_item_id()).await?;
        result.push((card, item));
    }
    Ok(result)
}

/// Builds a GetQueryDto for due cards with optional filters
async fn build_due_query(
    client: &HippocampusClient,
    item_type: Option<String>,
    tags: &[String],
) -> Result<GetQueryDto, Box<dyn std::error::Error>> {
    let item_type_id = match item_type {
        Some(ref name_or_id) => Some(resolve_item_type_id(client, name_or_id).await?),
        None => None,
    };
    let tag_ids = resolve_tag_ids(client, tags).await?;

    Ok(GetQueryDto {
        item_type_id,
        tag_ids,
        next_review_before: Some(tomorrow_midnight()),
        suspended_filter: SuspendedFilter::Exclude,
        ..Default::default()
    })
}

/// Executes a todo command
pub async fn execute(
    client: &HippocampusClient,
    cmd: TodoCommands,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        TodoCommands::Add {
            title,
            item_type,
            tag,
            data,
        } => {
            let item_type_id = resolve_item_type_id(client, &item_type).await?;
            let item_data: serde_json::Value = serde_json::from_str(&data)?;
            let item = client
                .create_item(item_type_id, title, item_data, 0.5)
                .await?;

            // Attach tags if specified
            if !tag.is_empty() {
                let tag_ids = resolve_tag_ids(client, &tag).await?;
                for tag_id in &tag_ids {
                    client.add_tag_to_item(&item.get_id(), tag_id).await?;
                }
            }

            output::print_item(&item, format);
        }

        TodoCommands::Due { item_type, tag } => {
            let query = build_due_query(client, item_type, &tag).await?;
            let cards = client.list_cards(&query).await?;
            let cards_with_items = fetch_cards_with_items(client, cards).await?;
            output::print_todo_cards(&cards_with_items, format);
        }

        TodoCommands::Completed { item_type } => {
            let item_type_id = match item_type {
                Some(ref name_or_id) => Some(resolve_item_type_id(client, name_or_id).await?),
                None => None,
            };
            let query = GetQueryDto {
                item_type_id,
                suspended_filter: SuspendedFilter::Only,
                suspended_after: Some(today_midnight()),
                ..Default::default()
            };
            let cards = client.list_cards(&query).await?;
            let cards_with_items = fetch_cards_with_items(client, cards).await?;
            output::print_todo_cards(&cards_with_items, format);
        }

        TodoCommands::Complete { card_id } => {
            client.suspend_card(&card_id, true).await?;
            output::print_success(&format!("Completed todo {}", card_id), format);
        }

        TodoCommands::Uncomplete { card_id } => {
            client.suspend_card(&card_id, false).await?;
            output::print_success(&format!("Uncompleted todo {}", card_id), format);
        }

        TodoCommands::Review { card_id, rating } => {
            let review = client.create_review(card_id, rating).await?;
            output::print_review(&review, format);
        }

        TodoCommands::Count { item_type, tag } => {
            let query = build_due_query(client, item_type, &tag).await?;
            let cards = client.list_cards(&query).await?;
            output::print_todo_count(cards.len(), format);
        }
    }
    Ok(())
}
