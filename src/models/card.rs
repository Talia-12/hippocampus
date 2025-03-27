use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::JsonValue;

/// Represents a card in the spaced repetition system
#[derive(Queryable, Selectable, Insertable, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::cards)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Card {
    /// Unique identifier for the card (UUID v4 as string)
    id: String,
    
    /// The ID of the item this card belongs to
    item_id: String,
    
    /// The index of this card within its item
    card_index: i32,
    
    /// When this card should next be reviewed
    next_review: Option<NaiveDateTime>,
    
    /// When this card was last reviewed
    last_review: Option<NaiveDateTime>,
    
    /// JSON data for the scheduler, stored as TEXT
    scheduler_data: Option<JsonValue>,
}

impl Card {
    /// Creates a new card for an item
    ///
    /// ### Arguments
    ///
    /// * `item_id` - The ID of the item this card belongs to
    /// * `card_index` - The index of this card within its item
    ///
    /// ### Returns
    ///
    /// A new `Card` instance with the specified item ID and card index
    pub fn new(item_id: String, card_index: i32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            item_id,
            card_index,
            next_review: None,
            last_review: None,
            scheduler_data: None,
        }
    }
    
    /// Creates a new card with all fields specified
    ///
    /// ### Arguments
    ///
    /// * `id` - The unique identifier for the card
    /// * `item_id` - The ID of the item this card belongs to
    /// * `card_index` - The index of this card within its item
    /// * `next_review` - When this card should next be reviewed
    /// * `last_review` - When this card was last reviewed
    /// * `scheduler_data` - JSON data for the scheduler
    ///
    /// ### Returns
    ///
    /// A new `Card` instance with the specified fields
    pub fn new_with_fields(
        id: String,
        item_id: String,
        card_index: i32,
        next_review: Option<DateTime<Utc>>,
        last_review: Option<DateTime<Utc>>,
        scheduler_data: Option<JsonValue>
    ) -> Self {
        Self {
            id,
            item_id,
            card_index,
            next_review: next_review.map(|dt| dt.naive_utc()),
            last_review: last_review.map(|dt| dt.naive_utc()),
            scheduler_data,
        }
    }
    
    /// Gets the card's ID
    ///
    /// ### Returns
    ///
    /// The unique identifier of the card
    pub fn get_id(&self) -> String {
        self.id.clone()
    }
    
    /// Gets the card's item ID
    ///
    /// ### Returns
    ///
    /// The ID of the item this card belongs to
    pub fn get_item_id(&self) -> String {
        self.item_id.clone()
    }
    
    /// Sets the card's item ID
    ///
    /// ### Arguments
    ///
    /// * `item_id` - The new item ID for the card
    pub fn set_item_id(&mut self, item_id: String) {
        self.item_id = item_id;
    }
    
    /// Gets the card's index within its item
    ///
    /// ### Returns
    ///
    /// The index of this card within its item
    pub fn get_card_index(&self) -> i32 {
        self.card_index
    }
    
    /// Sets the card's index within its item
    ///
    /// ### Arguments
    ///
    /// * `card_index` - The new index for the card
    pub fn set_card_index(&mut self, card_index: i32) {
        self.card_index = card_index;
    }
    
    /// Gets the card's next review timestamp as a DateTime<Utc>
    ///
    /// ### Returns
    ///
    /// The timestamp when this card should next be reviewed, or None if not scheduled
    pub fn get_next_review(&self) -> Option<DateTime<Utc>> {
        self.next_review.map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
    }
    
    /// Gets the card's raw next review timestamp
    ///
    /// ### Returns
    ///
    /// The raw NaiveDateTime when this card should next be reviewed, or None if not scheduled
    pub fn get_next_review_raw(&self) -> Option<NaiveDateTime> {
        self.next_review
    }
    
    /// Sets the card's next review timestamp
    ///
    /// ### Arguments
    ///
    /// * `next_review` - The new next review timestamp for the card
    pub fn set_next_review(&mut self, next_review: Option<DateTime<Utc>>) {
        self.next_review = next_review.map(|dt| dt.naive_utc());
    }
    
    /// Gets the card's last review timestamp as a DateTime<Utc>
    ///
    /// ### Returns
    ///
    /// The timestamp when this card was last reviewed, or None if never reviewed
    pub fn get_last_review(&self) -> Option<DateTime<Utc>> {
        self.last_review.map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
    }
    
    /// Gets the card's raw last review timestamp
    ///
    /// ### Returns
    ///
    /// The raw NaiveDateTime when this card was last reviewed, or None if never reviewed
    pub fn get_last_review_raw(&self) -> Option<NaiveDateTime> {
        self.last_review
    }
    
    /// Sets the card's last review timestamp
    ///
    /// ### Arguments
    ///
    /// * `last_review` - The new last review timestamp for the card
    pub fn set_last_review(&mut self, last_review: Option<DateTime<Utc>>) {
        self.last_review = last_review.map(|dt| dt.naive_utc());
    }
    
    /// Gets the card's scheduler data
    ///
    /// ### Returns
    ///
    /// The JSON data for the scheduler, or None if not set
    pub fn get_scheduler_data(&self) -> Option<JsonValue> {
        self.scheduler_data.clone()
    }
    
    /// Sets the card's scheduler data
    ///
    /// ### Arguments
    ///
    /// * `data` - The new scheduler data for the card
    pub fn set_scheduler_data(&mut self, data: Option<JsonValue>) {
        self.scheduler_data = data;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_card_new() {
        let item_id = Uuid::new_v4().to_string();
        let card_index = 1;
        
        let card = Card::new(item_id.clone(), card_index);
        
        assert_eq!(card.get_item_id(), item_id);
        assert_eq!(card.get_card_index(), card_index);
        assert!(Uuid::parse_str(&card.get_id()).is_ok());
        assert_eq!(card.get_next_review(), None);
        assert_eq!(card.get_last_review(), None);
        assert_eq!(card.get_scheduler_data(), None);
    }
    
    #[test]
    fn test_card_scheduler_data() {
        let item_id = Uuid::new_v4().to_string();
        let card_index = 1;
        let scheduler_data = Some(JsonValue(json!({
            "ease_factor": 2.5,
            "interval": 1,
            "repetitions": 0,
        })));
        
        let mut card = Card::new(item_id, card_index);
        card.set_scheduler_data(scheduler_data.clone());
        
        assert_eq!(card.get_scheduler_data(), scheduler_data);
    }
} 