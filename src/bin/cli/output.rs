use chrono::{DateTime, Utc};
use clap::ValueEnum;
use hippocampus::models::{Card, Item, ItemType, Review, Tag};

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

/// Prints a single item type in the specified format
pub fn print_item_type(item_type: &ItemType, format: OutputFormat) {
    match format {
        OutputFormat::Human => {
            println!("ID:      {}", item_type.get_id());
            println!("Name:    {}", item_type.get_name());
            println!("Created: {}", item_type.get_created_at());
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(item_type).unwrap());
        }
        OutputFormat::Waybar => {
            println!("{}", serde_json::to_string(item_type).unwrap());
        }
    }
}

/// Prints a list of items in the specified format
pub fn print_items(items: &[Item], format: OutputFormat) {
    match format {
        OutputFormat::Human => {
            if items.is_empty() {
                println!("No items found.");
                return;
            }
            for item in items {
                println!("{}\t{}", item.get_id(), item.get_title());
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(items).unwrap());
        }
        OutputFormat::Waybar => {
            println!("{}", serde_json::to_string(items).unwrap());
        }
    }
}

/// Prints a single item in the specified format
pub fn print_item(item: &Item, format: OutputFormat) {
    match format {
        OutputFormat::Human => {
            println!("ID:        {}", item.get_id());
            println!("Type:      {}", item.get_item_type());
            println!("Title:     {}", item.get_title());
            println!("Data:      {}", item.get_data().0);
            println!("Created:   {}", item.get_created_at());
            println!("Updated:   {}", item.get_updated_at());
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(item).unwrap());
        }
        OutputFormat::Waybar => {
            println!("{}", serde_json::to_string(item).unwrap());
        }
    }
}

/// Prints a list of cards in the specified format
pub fn print_cards(cards: &[Card], format: OutputFormat) {
    match format {
        OutputFormat::Human => {
            if cards.is_empty() {
                println!("No cards found.");
                return;
            }
            for card in cards {
                let suspended = match card.get_suspended() {
                    Some(dt) => format!("suspended {}", dt.format("%Y-%m-%d %H:%M")),
                    None => "active".to_string(),
                };
                println!(
                    "{}\titem:{}\tpri:{:.2}\tnext:{}\t{}",
                    card.get_id(),
                    card.get_item_id(),
                    card.get_priority(),
                    card.get_next_review().format("%Y-%m-%d %H:%M"),
                    suspended,
                );
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(cards).unwrap());
        }
        OutputFormat::Waybar => {
            println!("{}", serde_json::to_string(cards).unwrap());
        }
    }
}

/// Prints a single card in the specified format
pub fn print_card(card: &Card, format: OutputFormat) {
    match format {
        OutputFormat::Human => {
            println!("ID:          {}", card.get_id());
            println!("Item ID:     {}", card.get_item_id());
            println!("Card Index:  {}", card.get_card_index());
            println!("Priority:    {:.2}", card.get_priority());
            println!("Next Review: {}", card.get_next_review());
            match card.get_last_review() {
                Some(dt) => println!("Last Review: {}", dt),
                None => println!("Last Review: never"),
            }
            match card.get_suspended() {
                Some(dt) => println!("Suspended:   {}", dt),
                None => println!("Suspended:   no"),
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(card).unwrap());
        }
        OutputFormat::Waybar => {
            println!("{}", serde_json::to_string(card).unwrap());
        }
    }
}

/// Prints a list of reviews in the specified format
pub fn print_reviews(reviews: &[Review], format: OutputFormat) {
    match format {
        OutputFormat::Human => {
            if reviews.is_empty() {
                println!("No reviews found.");
                return;
            }
            for review in reviews {
                println!(
                    "{}\tcard:{}\trating:{}\t{}",
                    review.get_id(),
                    review.get_card_id(),
                    review.get_rating(),
                    review.get_review_timestamp().format("%Y-%m-%d %H:%M"),
                );
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(reviews).unwrap());
        }
        OutputFormat::Waybar => {
            println!("{}", serde_json::to_string(reviews).unwrap());
        }
    }
}

/// Prints a single review in the specified format
pub fn print_review(review: &Review, format: OutputFormat) {
    match format {
        OutputFormat::Human => {
            println!("ID:        {}", review.get_id());
            println!("Card ID:   {}", review.get_card_id());
            println!("Rating:    {}", review.get_rating());
            println!("Timestamp: {}", review.get_review_timestamp());
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(review).unwrap());
        }
        OutputFormat::Waybar => {
            println!("{}", serde_json::to_string(review).unwrap());
        }
    }
}

/// Prints a list of tags in the specified format
pub fn print_tags(tags: &[Tag], format: OutputFormat) {
    match format {
        OutputFormat::Human => {
            if tags.is_empty() {
                println!("No tags found.");
                return;
            }
            for tag in tags {
                let vis = if tag.get_visible() { "visible" } else { "hidden" };
                println!("{}\t{}\t{}", tag.get_id(), tag.get_name(), vis);
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(tags).unwrap());
        }
        OutputFormat::Waybar => {
            println!("{}", serde_json::to_string(tags).unwrap());
        }
    }
}

/// Prints a single tag in the specified format
pub fn print_tag(tag: &Tag, format: OutputFormat) {
    match format {
        OutputFormat::Human => {
            println!("ID:      {}", tag.get_id());
            println!("Name:    {}", tag.get_name());
            println!("Visible: {}", tag.get_visible());
            println!("Created: {}", tag.get_created_at());
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(tag).unwrap());
        }
        OutputFormat::Waybar => {
            println!("{}", serde_json::to_string(tag).unwrap());
        }
    }
}

/// Prints next review possibilities in the specified format
pub fn print_next_reviews(
    next_reviews: &[(DateTime<Utc>, serde_json::Value)],
    format: OutputFormat,
) {
    match format {
        OutputFormat::Human => {
            if next_reviews.is_empty() {
                println!("No next reviews available.");
                return;
            }
            for (i, (dt, data)) in next_reviews.iter().enumerate() {
                println!(
                    "Rating {}: {} ({})",
                    i + 1,
                    dt.format("%Y-%m-%d %H:%M"),
                    data
                );
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(next_reviews).unwrap());
        }
        OutputFormat::Waybar => {
            println!("{}", serde_json::to_string(next_reviews).unwrap());
        }
    }
}

/// Prints a simple success message (for operations that don't return data)
pub fn print_success(message: &str, format: OutputFormat) {
    match format {
        OutputFormat::Human => println!("{}", message),
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({"status": "ok", "message": message}))
                    .unwrap()
            );
        }
        OutputFormat::Waybar => {
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({"status": "ok", "message": message}))
                    .unwrap()
            );
        }
    }
}
