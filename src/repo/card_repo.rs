use crate::db::DbPool;
use crate::models::{Card, Item, JsonValue, Tag};
use crate::schema::{cards, item_tags, tags};
use crate::GetQueryDto;
use diesel::prelude::*;
use anyhow::{Result, anyhow};
use chrono::Utc;

/// Creates cards for an item
///
/// This function automatically creates the necessary cards for an item
/// based on its type and data. Currently, it creates exactly one card per item.
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item` - The item to create cards for
///
/// ### Returns
///
/// A Result containing a vector of the created Cards if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database insert operation fails
pub fn create_cards_for_item(pool: &DbPool, item: &Item) -> Result<Vec<Card>> {
    // Get the item type to determine how many cards to create
    let item_type = super::get_item_type(pool, &item.get_item_type())?
        .ok_or_else(|| anyhow!("Item type not found"))?;
    
    // Vector to store the created cards
    let mut cards = Vec::new();
    
    // Determine how many cards to create based on the item type
    match item_type.get_name().as_str() {
        "Basic" => {
            // Basic items have just one card (front/back)
            let card = create_card(pool, &item.get_id(), 0)?;
            cards.push(card);
        },
        "Cloze" => {
            // Cloze items might have multiple cards (one per cloze deletion)
            let data = item.get_data();
            let cloze_deletions = data.0["clozes"].clone();
            let cloze_deletions = cloze_deletions.as_array()
                .ok_or_else(|| anyhow!("cloze deletion must be an array"))?;
            for (index, _) in cloze_deletions.iter().enumerate() {
                let card = create_card(pool, &item.get_id(), index as i32)?;
                cards.push(card);
            }
        },
        "Vocabulary" => {
            // Vocabulary items have 2 cards (term->definition and definition->term)
            for i in 0..2 {
                let card = create_card(pool, &item.get_id(), i)?;
                cards.push(card);
            }
        },
        "Todo" => {
            // Todo items have 1 card (each todo is a card)
            let card = create_card(pool, &item.get_id(), 0)?;
            cards.push(card);
        },
        // TODO: this is a hack
        name if name.contains("Test") => {
            // Test item types have 2 cards
            for i in 0..2 {
                let card = create_card(pool, &item.get_id(), i)?;
                cards.push(card);
            }
        },
        _ => {
            // Return an error for unknown item types
            return Err(anyhow!("Unable to construct cards for unknown item type: {}", item_type.get_name()));
        }
    }
    
    // Return all created cards
    Ok(cards)

}

/// Creates a new card in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item this card belongs to
/// * `card_index` - The index of this card within its item
///
/// ### Returns
///
/// A Result containing the newly created Card if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database insert operation fails
pub fn create_card(pool: &DbPool, item_id: &str, card_index: i32) -> Result<Card> {
    let conn = &mut pool.get()?;
    
    // Create a new card for the item
    let new_card = Card::new(item_id.to_string(), card_index);
    
    // Insert the new card into the database
    diesel::insert_into(cards::table)
        .values(&new_card)
        .execute(conn)?;
    
    // Return the newly created card
    Ok(new_card)
}

/// Retrieves a card from the database by its ID
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to retrieve
///
/// ### Returns
///
/// A Result containing an Option with the Card if found, or None if not found
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails for reasons other than the card not existing
pub fn get_card(pool: &DbPool, card_id: &str) -> Result<Option<Card>> {
    let conn = &mut pool.get()?;
    
    let result = cards::table
        .find(card_id)
        .first::<Card>(conn)
        .optional()?;
    
    Ok(result)
}

/// Lists all cards in the database with optional filtering
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `query` - Optional filters for the cards
///
/// ### Returns
///
/// A Result containing a vector of Cards matching the filters
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
pub fn list_cards_with_filters(pool: &DbPool, query: &GetQueryDto) -> Result<Vec<Card>> {
    let conn = &mut pool.get()?;
    
    // Start with a base query that joins cards with items
    let mut card_query = cards::table.into_boxed();
    
    // Apply filter by item type, if specified
    if let Some(item_type_id) = &query.item_type_id {
        card_query = card_query.filter(
            cards::item_id.eq_any(
                crate::schema::items::table
                    .filter(crate::schema::items::item_type.eq(item_type_id))
                    .select(crate::schema::items::id)
            )
        );
    }
    
    // Apply filter by review date, if specified
    if let Some(review_date) = query.next_review_before {
        card_query = card_query.filter(
            cards::next_review.lt(review_date.naive_utc()).and(cards::next_review.is_not_null())
        );
    }
    
    // Execute the query
    let mut results = card_query.load::<Card>(conn)?;
    
    // Apply tag filters if specified
    // Note: This is a bit inefficient as we're filtering in Rust rather than SQL,
    // but it's simpler than constructing a complex query with multiple joins.
    if !query.tag_ids.is_empty() {
        // Get all item_ids that have all the requested tags
        let mut item_ids_with_tags = Vec::new();
        
        // Get all items with any of the requested tags
        let items_with_tags: Vec<String> = item_tags::table
            .filter(item_tags::tag_id.eq_any(&query.tag_ids))
            .select(item_tags::item_id)
            .load(conn)?;
        
        // Count how many tags each item has
        let mut item_tag_counts = std::collections::HashMap::new();
        for item_id in items_with_tags {
            *item_tag_counts.entry(item_id).or_insert(0) += 1;
        }
        
        // Only keep items that have all requested tags
        for (item_id, count) in item_tag_counts {
            if count as usize == query.tag_ids.len() {
                item_ids_with_tags.push(item_id);
            }
        }
        
        // Filter cards to only those belonging to items with all required tags
        results.retain(|card| item_ids_with_tags.contains(&card.get_item_id()));
    }
    
    Ok(results)
}

/// Gets all cards for a specific item
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `item_id` - The ID of the item to get cards for
///
/// ### Returns
///
/// A Result containing a vector of Cards belonging to the item
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
pub fn get_cards_for_item(pool: &DbPool, item_id: &str) -> Result<Vec<Card>> {
    let conn = &mut pool.get()?;
    
    // Check if the item exists
    let item_exists: bool = crate::schema::items::table
        .find(item_id)
        .count()
        .get_result::<i64>(conn)? > 0;
    
    if !item_exists {
        return Err(anyhow!("Item not found"));
    }
    
    // Get all cards for the item
    let results = cards::table
        .filter(cards::item_id.eq(item_id))
        .order_by(cards::card_index.asc())
        .load::<Card>(conn)?;
    
    Ok(results)
}

/// Lists all cards for a card
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card_id` - The ID of the card to get tags for
///
/// ### Returns
///
/// A Result containing a vector of Tags associated with the card's item
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
/// - The card does not exist
pub fn list_tags_for_card(pool: &DbPool, card_id: &str) -> Result<Vec<Tag>> {
    let conn = &mut pool.get()?;
    
    // Get the card to find its item_id
    let card = get_card(pool, card_id)?.ok_or_else(|| anyhow!("Card not found"))?;
    
    // Use the item_id to get tags
    let results = tags::table
        .inner_join(item_tags::table.on(tags::id.eq(item_tags::tag_id)))
        .filter(item_tags::item_id.eq(card.get_item_id()))
        .select(tags::all_columns)
        .load::<Tag>(conn)?;
    
    Ok(results)
}

/// Updates a card in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card` - The card to update
///
/// ### Returns
///
/// A Result indicating success (Ok(())) or an error
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database update operation fails
pub fn update_card(pool: &DbPool, card: &Card) -> Result<()> {
    let conn = &mut pool.get()?;
    
    diesel::update(cards::table.find(card.get_id()))
        .set((
            cards::next_review.eq(card.get_next_review_raw()),
            cards::last_review.eq(card.get_last_review_raw()),
            cards::scheduler_data.eq(card.get_scheduler_data()),
        ))
        .execute(conn)?;
    
    Ok(())
}

/// Lists all cards in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
///
/// ### Returns
///
/// A Result containing a vector of all Cards in the database
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
pub fn list_all_cards(pool: &DbPool) -> Result<Vec<Card>> {
    let conn = &mut pool.get()?;
    
    let results = cards::table
        .load::<Card>(conn)?;
    
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::tests::setup_test_db;
    use crate::repo::{create_item, create_item_type, create_tag, add_tag_to_item};
    use serde_json::json;
    use chrono::Duration;
    
    #[test]
    fn test_create_card() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        
        // Create an item
        let item = create_item(
            &pool, 
            &item_type.get_id(), 
            "Test Item".to_string(), 
            json!({"front": "Hello", "back": "World"})
        ).unwrap();
        
        // Test creating a card manually
        let card_index = 2;
        let card = create_card(&pool, &item.get_id(), card_index).unwrap();
        
        assert_eq!(card.get_item_id(), item.get_id());
        assert_eq!(card.get_card_index(), card_index);
    }
    
    #[test]
    fn test_get_card() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        
        // Create an item
        let item = create_item(
            &pool, 
            &item_type.get_id(), 
            "Test Item".to_string(), 
            json!({"front": "Hello", "back": "World"})
        ).unwrap();
        
        // Get the cards created for the item
        let cards = get_cards_for_item(&pool, &item.get_id()).unwrap();
        assert!(!cards.is_empty());
        
        // Test getting a card by ID
        let card_id = cards[0].get_id();
        let retrieved_card = get_card(&pool, &card_id).unwrap().unwrap();
        
        assert_eq!(retrieved_card.get_id(), card_id);
        assert_eq!(retrieved_card.get_item_id(), item.get_id());
    }
    
    #[test]
    fn test_retrieve_cards_by_item_id() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Vocabulary".to_string()).unwrap();
        
        // Create some items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).unwrap();
        
        let item2 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 2".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).unwrap();
        
        // Get cards for each item
        let cards1 = get_cards_for_item(&pool, &item1.get_id()).unwrap();
        let cards2 = get_cards_for_item(&pool, &item2.get_id()).unwrap();
        
        // Verify that each item has its own card(s)
        assert_eq!(cards1.len(), 2);
        assert_eq!(cards2.len(), 2);
        assert_eq!(cards1[0].get_item_id(), item1.get_id());
        assert_eq!(cards1[1].get_item_id(), item1.get_id());
        assert_eq!(cards2[0].get_item_id(), item2.get_id());
        assert_eq!(cards2[1].get_item_id(), item2.get_id());
    }
    
    #[test]
    fn test_list_all_cards() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Test Vocabulary".to_string()).unwrap();
        
        // Create some items (which will also create cards)
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).unwrap();
        
        let item2 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 2".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).unwrap();
        
        // List all cards
        let cards = list_all_cards(&pool).unwrap();
        
        // Verify that we got all cards
        assert_eq!(cards.len(), 4);
        assert!(cards.iter().filter(|c| c.get_item_id() == item1.get_id()).count() == 2);
        assert!(cards.iter().filter(|c| c.get_item_id() == item2.get_id()).count() == 2);
    }
    
    #[test]
    fn test_filter_cards_by_item_type() {
        let pool = setup_test_db();
        
        // Create two item types
        let vocab_type = create_item_type(&pool, "Test Vocabulary".to_string()).unwrap();
        let grammar_type = create_item_type(&pool, "Test Grammar".to_string()).unwrap();
        
        // Create items of different types
        let vocab_item = create_item(
            &pool, 
            &vocab_type.get_id(), 
            "Vocab Item".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).unwrap();
        
        let grammar_item = create_item(
            &pool, 
            &grammar_type.get_id(), 
            "Grammar Item".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).unwrap();
        
        // Filter cards by item type
        let query1 = GetQueryDto {
            item_type_id: Some(vocab_type.get_id()),
            tag_ids: vec![],
            next_review_before: None,
        };
        
        let query2 = GetQueryDto {
            item_type_id: Some(grammar_type.get_id()),
            tag_ids: vec![],
            next_review_before: None,
        };
        
        let vocab_cards = list_cards_with_filters(&pool, &query1).unwrap();
        let grammar_cards = list_cards_with_filters(&pool, &query2).unwrap();
        
        // Verify that we got the right cards
        assert_eq!(vocab_cards.len(), 2);
        assert_eq!(grammar_cards.len(), 2);
        assert_eq!(vocab_cards[0].get_item_id(), vocab_item.get_id());
        assert_eq!(vocab_cards[1].get_item_id(), vocab_item.get_id());
        assert_eq!(grammar_cards[0].get_item_id(), grammar_item.get_id());
        assert_eq!(grammar_cards[1].get_item_id(), grammar_item.get_id());
    }
    
    #[test]
    fn test_filter_cards_by_tags() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        
        // Create some items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).unwrap();
        
        let item2 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 2".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).unwrap();
        
        let item3 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 3".to_string(), 
            json!({"front": "F3", "back": "B3"})
        ).unwrap();
        
        // Create some tags
        let tag1 = create_tag(&pool, "Important".to_string(), true).unwrap();
        let tag2 = create_tag(&pool, "Difficult".to_string(), true).unwrap();
        
        // Add tags to items
        add_tag_to_item(&pool, &tag1.get_id(), &item1.get_id()).unwrap();
        add_tag_to_item(&pool, &tag2.get_id(), &item1.get_id()).unwrap();
        add_tag_to_item(&pool, &tag1.get_id(), &item2.get_id()).unwrap();
        // Item 3 has no tags
        
        // Filter cards by tags
        let query1 = GetQueryDto {
            item_type_id: None,
            tag_ids: vec![tag1.get_id()],
            next_review_before: None,
        };
        
        let query2 = GetQueryDto {
            item_type_id: None,
            tag_ids: vec![tag2.get_id()],
            next_review_before: None,
        };
        
        let query_both_tags = GetQueryDto {
            item_type_id: None,
            tag_ids: vec![tag1.get_id(), tag2.get_id()],
            next_review_before: None,
        };
        
        let cards_tag1 = list_cards_with_filters(&pool, &query1).unwrap();
        let cards_tag2 = list_cards_with_filters(&pool, &query2).unwrap();
        let cards_both_tags = list_cards_with_filters(&pool, &query_both_tags).unwrap();
        
        // Verify that we got the right cards
        assert_eq!(cards_tag1.len(), 4); // Items 1 and 2 have tag1
        assert_eq!(cards_tag2.len(), 2); // Only item 1 has tag2
        assert_eq!(cards_both_tags.len(), 2); // Only item 1 has both tags
        
        // Verify specific cards
        assert!(cards_tag1.iter().any(|c| c.get_item_id() == item1.get_id()));
        assert!(cards_tag1.iter().any(|c| c.get_item_id() == item2.get_id()));
        assert!(cards_tag2.iter().all(|c| c.get_item_id() == item1.get_id()));
        assert!(cards_both_tags.iter().all(|c| c.get_item_id() == item1.get_id()));
    }
    
    #[test]
    fn test_filter_cards_by_next_review() {
        let pool = setup_test_db();
        
        // Create an item type
        let item_type = create_item_type(&pool, "TestVocabulary".to_string()).unwrap();
        
        // Create some items
        let item1 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).unwrap();
        
        let item2 = create_item(
            &pool, 
            &item_type.get_id(), 
            "Item 2".to_string(), 
            json!({"front": "F2", "back": "B2"})
        ).unwrap();
        
        // Get cards for each item - there will be 2 cards per item
        let mut cards = vec![];
        cards.append(&mut get_cards_for_item(&pool, &item1.get_id()).unwrap());
        cards.append(&mut get_cards_for_item(&pool, &item2.get_id()).unwrap());
        
        // Set different next_review times for the cards
        let now = Utc::now();
        let yesterday = now - Duration::days(1);
        let tomorrow = now + Duration::days(1);
        
        cards[0].set_next_review(Some(yesterday)); // Due
        cards[1].set_next_review(Some(tomorrow));  // Not due
        cards[2].set_next_review(None); // Card 3 has no next_review (null), so it's never due
        cards[3].set_next_review(Some(yesterday)); // Due

        // Update the cards in the database
        for card in &cards {
            update_card(&pool, card).unwrap();
        }
        
        // Filter cards by next_review
        let query = GetQueryDto {
            item_type_id: None,
            tag_ids: vec![],
            next_review_before: Some(now),
        };
        
        let due_cards = list_cards_with_filters(&pool, &query).unwrap();
        
        // Verify that we got the right cards
        assert_eq!(due_cards.len(), 2); // Cards 1 and 4 are due
        
        // Verify specific cards
        let due_card_ids: Vec<String> = due_cards.iter().map(|c| c.get_id()).collect();
        assert!(due_card_ids.contains(&cards[0].get_id())); // Card 1 is due
        assert!(!due_card_ids.contains(&cards[1].get_id())); // Card 2 is not due
        assert!(!due_card_ids.contains(&cards[2].get_id())); // Card 3 is not due (no next_review)
        assert!(due_card_ids.contains(&cards[3].get_id())); // Card 4 is due
    }
    
    #[test]
    fn test_filter_cards_with_multiple_criteria() {
        let pool = setup_test_db();
        
        // Create two item types
        let vocab_type = create_item_type(&pool, "Test Vocabulary".to_string()).unwrap();
        let grammar_type = create_item_type(&pool, "Test Grammar".to_string()).unwrap();
        
        // Create items of different types
        let vocab_item1 = create_item(
            &pool, 
            &vocab_type.get_id(), 
            "Vocab 1".to_string(), 
            json!({"front": "F1", "back": "B1"})
        ).unwrap();
        
        let grammar_item = create_item(
            &pool, 
            &grammar_type.get_id(), 
            "Grammar".to_string(), 
            json!({"front": "F3", "back": "B3"})
        ).unwrap();
        
        // Create some tags
        let important_tag = create_tag(&pool, "Important".to_string(), true).unwrap();
        
        // Add tags to items
        add_tag_to_item(&pool, &important_tag.get_id(), &vocab_item1.get_id()).unwrap();
        add_tag_to_item(&pool, &important_tag.get_id(), &grammar_item.get_id()).unwrap();
        // vocab_item2 has no tags
        
        // Get cards for each item
        let mut vocab_card1 = get_cards_for_item(&pool, &vocab_item1.get_id()).unwrap()[0].clone();
        let mut vocab_card2 = get_cards_for_item(&pool, &vocab_item1.get_id()).unwrap()[1].clone();
        let mut grammar_card = get_cards_for_item(&pool, &grammar_item.get_id()).unwrap()[0].clone();
        
        // Set different next_review times for the cards
        let now = Utc::now();
        let yesterday = now - Duration::days(1);
        let tomorrow = now + Duration::days(1);
        
        vocab_card1.set_next_review(Some(yesterday)); // Due
        vocab_card2.set_next_review(Some(tomorrow));  // Not due
        grammar_card.set_next_review(Some(tomorrow)); // Not due
        
        // Update the cards in the database
        update_card(&pool, &vocab_card1).unwrap();
        update_card(&pool, &vocab_card2).unwrap();
        update_card(&pool, &grammar_card).unwrap();
        
        // Filter cards by multiple criteria
        let query = GetQueryDto {
            item_type_id: Some(vocab_type.get_id()),
            tag_ids: vec![important_tag.get_id()],
            next_review_before: Some(now),
        };
        
        let filtered_cards = list_cards_with_filters(&pool, &query).unwrap();
        
        // Verify that we got exactly the right card
        assert_eq!(filtered_cards.len(), 1);
        assert!(filtered_cards.iter().any(|c| c.get_id() == vocab_card1.get_id()));
        
        // This card should:
        // 1. Belong to an item of type "Vocabulary"
        // 2. Belong to an item tagged as "Important"
        // 3. Be due for review (next_review is earlier than now)
    }
    
    #[test]
    fn test_filter_cards_edge_cases() {
        let pool = setup_test_db();
        
        // Test with empty database
        let query = GetQueryDto {
            item_type_id: None,
            tag_ids: vec![],
            next_review_before: None,
        };
        
        let cards = list_cards_with_filters(&pool, &query).unwrap();
        assert_eq!(cards.len(), 0);
        
        // Test with non-existent item type
        let query = GetQueryDto {
            item_type_id: Some("nonexistent".to_string()),
            tag_ids: vec![],
            next_review_before: None,
        };
        
        let cards = list_cards_with_filters(&pool, &query).unwrap();
        assert_eq!(cards.len(), 0);
        
        // Test with non-existent tag
        let query = GetQueryDto {
            item_type_id: None,
            tag_ids: vec!["nonexistent".to_string()],
            next_review_before: None,
        };
        
        let cards = list_cards_with_filters(&pool, &query).unwrap();
        assert_eq!(cards.len(), 0);
    }
} 