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

/// Bundled output configuration passed to all print functions
#[derive(Debug, Clone, Copy)]
pub struct OutputConfig {
    /// The output format
    pub format: OutputFormat,
    /// When true, print minimal output (just IDs or counts)
    pub quiet: bool,
}

/// Prints a list of item types in the specified format
pub fn print_item_types(item_types: &[ItemType], config: &OutputConfig) {
    match config.format {
        OutputFormat::Human => {
            if item_types.is_empty() {
                if !config.quiet {
                    println!("No item types found.");
                }
                return;
            }
            if config.quiet {
                for it in item_types {
                    println!("{}", it.get_id());
                }
                return;
            }
            let max_id = item_types.iter().map(|t| t.get_id().len()).max().unwrap_or(2);
            println!("{:<width$}  NAME", "ID", width = max_id);
            for it in item_types {
                println!("{:<width$}  {}", it.get_id(), it.get_name(), width = max_id);
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
pub fn print_item_type(item_type: &ItemType, config: &OutputConfig) {
    match config.format {
        OutputFormat::Human => {
            if config.quiet {
                println!("{}", item_type.get_id());
                return;
            }
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
pub fn print_items(items: &[Item], config: &OutputConfig) {
    match config.format {
        OutputFormat::Human => {
            if items.is_empty() {
                if !config.quiet {
                    println!("No items found.");
                }
                return;
            }
            if config.quiet {
                for item in items {
                    println!("{}", item.get_id());
                }
                return;
            }
            let max_id = items.iter().map(|i| i.get_id().len()).max().unwrap_or(2);
            let max_title = items
                .iter()
                .map(|i| i.get_title().len())
                .max()
                .unwrap_or(5);
            println!(
                "{:<id_w$}  {:<title_w$}",
                "ID",
                "TITLE",
                id_w = max_id,
                title_w = max_title,
            );
            for item in items {
                println!(
                    "{:<id_w$}  {:<title_w$}",
                    item.get_id(),
                    item.get_title(),
                    id_w = max_id,
                    title_w = max_title,
                );
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
pub fn print_item(item: &Item, config: &OutputConfig) {
    match config.format {
        OutputFormat::Human => {
            if config.quiet {
                println!("{}", item.get_id());
                return;
            }
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
pub fn print_cards(cards: &[Card], config: &OutputConfig) {
    match config.format {
        OutputFormat::Human => {
            if cards.is_empty() {
                if !config.quiet {
                    println!("No cards found.");
                }
                return;
            }
            if config.quiet {
                for card in cards {
                    println!("{}", card.get_id());
                }
                return;
            }
            let max_id = cards.iter().map(|c| c.get_id().len()).max().unwrap_or(2);
            let max_item = cards.iter().map(|c| c.get_item_id().len()).max().unwrap_or(4);
            println!(
                "{:<id_w$}  {:<item_w$}  {:>8}  {:<16}  {:>8}  STATUS",
                "ID",
                "ITEM",
                "PRIORITY",
                "NEXT REVIEW",
                "SORT",
                id_w = max_id,
                item_w = max_item,
            );
            for card in cards {
                let status = match card.get_suspended() {
                    Some(dt) => format!("suspended {}", dt.format("%Y-%m-%d %H:%M")),
                    None => "active".to_string(),
                };
                let sort_pos = match card.get_sort_position() {
                    Some(pos) => format!("{:.2}", pos),
                    None => "-".to_string(),
                };
                println!(
                    "{:<id_w$}  {:<item_w$}  {:>8.2}  {:<16}  {:>8}  {}",
                    card.get_id(),
                    card.get_item_id(),
                    card.get_priority(),
                    card.get_next_review().format("%Y-%m-%d %H:%M"),
                    sort_pos,
                    status,
                    id_w = max_id,
                    item_w = max_item,
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
pub fn print_card(card: &Card, config: &OutputConfig) {
    match config.format {
        OutputFormat::Human => {
            if config.quiet {
                println!("{}", card.get_id());
                return;
            }
            println!("ID:          {}", card.get_id());
            println!("Item ID:     {}", card.get_item_id());
            println!("Card Index:  {}", card.get_card_index());
            println!("Priority:    {:.2}", card.get_priority());
            println!("Next Review: {}", card.get_next_review());
            match card.get_last_review() {
                Some(dt) => println!("Last Review: {}", dt),
                None => println!("Last Review: never"),
            }
            match card.get_sort_position() {
                Some(pos) => println!("Sort Pos:    {:.2}", pos),
                None => println!("Sort Pos:    -"),
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

/// Prints a card from a raw JSON value (used when the server returns transformed card JSON)
pub fn print_card_json(card: &serde_json::Value, config: &OutputConfig) {
    match config.format {
        OutputFormat::Human => {
            if config.quiet {
                if let Some(id) = card.get("id").and_then(|v| v.as_str()) {
                    println!("{}", id);
                }
                return;
            }
            println!("{}", serde_json::to_string_pretty(card).unwrap());
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
pub fn print_reviews(reviews: &[Review], config: &OutputConfig) {
    match config.format {
        OutputFormat::Human => {
            if reviews.is_empty() {
                if !config.quiet {
                    println!("No reviews found.");
                }
                return;
            }
            if config.quiet {
                for review in reviews {
                    println!("{}", review.get_id());
                }
                return;
            }
            let max_id = reviews.iter().map(|r| r.get_id().len()).max().unwrap_or(2);
            let max_card = reviews
                .iter()
                .map(|r| r.get_card_id().len())
                .max()
                .unwrap_or(4);
            println!(
                "{:<id_w$}  {:<card_w$}  {:>6}  TIMESTAMP",
                "ID",
                "CARD",
                "RATING",
                id_w = max_id,
                card_w = max_card,
            );
            for review in reviews {
                println!(
                    "{:<id_w$}  {:<card_w$}  {:>6}  {}",
                    review.get_id(),
                    review.get_card_id(),
                    review.get_rating(),
                    review.get_review_timestamp().format("%Y-%m-%d %H:%M"),
                    id_w = max_id,
                    card_w = max_card,
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
pub fn print_review(review: &Review, config: &OutputConfig) {
    match config.format {
        OutputFormat::Human => {
            if config.quiet {
                println!("{}", review.get_id());
                return;
            }
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
pub fn print_tags(tags: &[Tag], config: &OutputConfig) {
    match config.format {
        OutputFormat::Human => {
            if tags.is_empty() {
                if !config.quiet {
                    println!("No tags found.");
                }
                return;
            }
            if config.quiet {
                for tag in tags {
                    println!("{}", tag.get_id());
                }
                return;
            }
            let max_id = tags.iter().map(|t| t.get_id().len()).max().unwrap_or(2);
            let max_name = tags.iter().map(|t| t.get_name().len()).max().unwrap_or(4);
            println!(
                "{:<id_w$}  {:<name_w$}  VISIBLE",
                "ID",
                "NAME",
                id_w = max_id,
                name_w = max_name,
            );
            for tag in tags {
                let vis = if tag.get_visible() { "yes" } else { "no" };
                println!(
                    "{:<id_w$}  {:<name_w$}  {}",
                    tag.get_id(),
                    tag.get_name(),
                    vis,
                    id_w = max_id,
                    name_w = max_name,
                );
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
pub fn print_tag(tag: &Tag, config: &OutputConfig) {
    match config.format {
        OutputFormat::Human => {
            if config.quiet {
                println!("{}", tag.get_id());
                return;
            }
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
    config: &OutputConfig,
) {
    match config.format {
        OutputFormat::Human => {
            if next_reviews.is_empty() {
                if !config.quiet {
                    println!("No next reviews available.");
                }
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
pub fn print_success(message: &str, config: &OutputConfig) {
    match config.format {
        OutputFormat::Human => {
            if !config.quiet {
                println!("{}", message);
            }
        }
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

/// Prints cards with their associated item titles for todo commands
pub fn print_todo_cards(cards_with_items: &[(Card, Option<Item>)], config: &OutputConfig) {
    match config.format {
        OutputFormat::Human => {
            if cards_with_items.is_empty() {
                if !config.quiet {
                    println!("No todos found.");
                }
                return;
            }
            if config.quiet {
                for (card, _) in cards_with_items {
                    println!("{}", card.get_id());
                }
                return;
            }
            let titles: Vec<String> = cards_with_items
                .iter()
                .map(|(_, item)| {
                    item.as_ref()
                        .map(|i| i.get_title())
                        .unwrap_or_else(|| "???".to_string())
                })
                .collect();
            let max_id = cards_with_items
                .iter()
                .map(|(c, _)| c.get_id().len())
                .max()
                .unwrap_or(2);
            let max_title = titles.iter().map(|t| t.len()).max().unwrap_or(5);
            println!(
                "{:<id_w$}  {:<title_w$}  {:<16}  {:>8}  SORT",
                "ID",
                "TITLE",
                "NEXT REVIEW",
                "PRIORITY",
                id_w = max_id,
                title_w = max_title,
            );
            for ((card, _), title) in cards_with_items.iter().zip(titles.iter()) {
                let sort_pos = match card.get_sort_position() {
                    Some(pos) => format!("{:.2}", pos),
                    None => "-".to_string(),
                };
                println!(
                    "{:<id_w$}  {:<title_w$}  {:<16}  {:>8.2}  {}",
                    card.get_id(),
                    title,
                    card.get_next_review().format("%Y-%m-%d %H:%M"),
                    card.get_priority(),
                    sort_pos,
                    id_w = max_id,
                    title_w = max_title,
                );
            }
        }
        OutputFormat::Json => {
            let data: Vec<serde_json::Value> = cards_with_items
                .iter()
                .map(|(card, item)| {
                    serde_json::json!({
                        "card": card,
                        "item": item,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&data).unwrap());
        }
        OutputFormat::Waybar => {
            print_waybar_todo_summary(cards_with_items);
        }
    }
}

/// Prints waybar-compatible JSON for todo summaries
///
/// Output: `{"text": "3", "tooltip": "3 todos due\n- Title 1\n- Title 2", "class": "has-items"}`
/// Empty: `{"text": "", "tooltip": "No todos due", "class": "empty"}`
fn print_waybar_todo_summary(cards_with_items: &[(Card, Option<Item>)]) {
    let count = cards_with_items.len();
    let (text, tooltip, class) = if count > 0 {
        let titles: Vec<String> = cards_with_items
            .iter()
            .map(|(_, item)| {
                item.as_ref()
                    .map(|i| i.get_title())
                    .unwrap_or_else(|| "???".to_string())
            })
            .collect();
        let tooltip_lines: Vec<String> = titles.iter().map(|t| format!("- {}", t)).collect();
        let tooltip = format!(
            "{} todos due\n{}",
            count,
            tooltip_lines.join("\n")
        );
        (count.to_string(), tooltip, "has-items")
    } else {
        (String::new(), "No todos due".to_string(), "empty")
    };
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "text": text,
            "tooltip": tooltip,
            "class": class,
        }))
        .unwrap()
    );
}
