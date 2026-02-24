use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::JsonValue;

/// Represents a card in the spaced repetition system
#[derive(Queryable, Selectable, Insertable, Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    next_review: NaiveDateTime,
    
    /// When this card was last reviewed
    last_review: Option<NaiveDateTime>,
    
    /// JSON data for the scheduler, stored as TEXT
    scheduler_data: Option<JsonValue>,

    /// The priority of the card, between 0 and 1
    priority: f32,

    /// When this card was suspended (or null if it isn't suspended)
    suspended: Option<NaiveDateTime>,

    /// Temporary sort position for client-driven review ordering
    sort_position: Option<f32>,

    /// Daily random offset applied to priority for shuffling similarly-prioritized cards
    #[serde(default)]
    priority_offset: f32,
}


impl Card {
    /// Creates a new card for an item
    ///
    /// ### Arguments
    ///
    /// * `item_id` - The ID of the item this card belongs to
    /// * `card_index` - The index of this card within its item
    /// * `next_review` - When this card should next be reviewed
    /// * `priority` - The priority of the card, between 0 and 1
    ///
    /// ### Returns
    ///
    /// A new `Card` instance with the specified item ID and card index
    pub fn new(item_id: String, card_index: i32, next_review: DateTime<Utc>, priority: f32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            item_id,
            card_index,
            next_review: next_review.naive_utc(),
            last_review: None,
            scheduler_data: None,
            priority,
            suspended: None,
            sort_position: None,
            priority_offset: 0.0,
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
        next_review: DateTime<Utc>,
        last_review: Option<DateTime<Utc>>,
        scheduler_data: Option<JsonValue>,
        priority: f32,
        suspended: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            id,
            item_id,
            card_index,
            next_review: next_review.naive_utc(),
            last_review: last_review.map(|dt| dt.naive_utc()),
            scheduler_data,
            priority,
            suspended: suspended.map(|dt| dt.naive_utc()),
            sort_position: None,
            priority_offset: 0.0,
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
    pub fn get_next_review(&self) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(self.next_review, Utc)
    }
    
    /// Gets the card's raw next review timestamp
    ///
    /// ### Returns
    ///
    /// The raw NaiveDateTime when this card should next be reviewed, or None if not scheduled
    pub fn get_next_review_raw(&self) -> NaiveDateTime {
        self.next_review
    }
    
    /// Sets the card's next review timestamp
    ///
    /// ### Arguments
    ///
    /// * `next_review` - The new next review timestamp for the card
    pub fn set_next_review(&mut self, next_review: DateTime<Utc>) {
        self.next_review = next_review.naive_utc();
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

    /// Gets the card's priority
    ///
    /// ### Returns
    ///
    /// The priority of the card, between 0 and 1
    pub fn get_priority(&self) -> f32 {
        self.priority
    }

    /// Sets the card's priority
    ///
    /// ### Arguments
    ///
    /// * `priority` - The new priority for the card
    pub fn set_priority(&mut self, priority: f32) {
        self.priority = priority;
    }


    /// Gets the card's suspend timestamp as a DateTime<Utc>
    ///
    /// ### Returns
    ///
    /// The timestamp when this card was suspended, or None if never suspended
    pub fn get_suspended(&self) -> Option<DateTime<Utc>> {
        self.suspended.map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
    }
    
    /// Gets the card's raw suspended timestamp
    ///
    /// ### Returns
    ///
    /// The raw NaiveDateTime when this card was suspended, or None if never suspended
    pub fn get_suspended_raw(&self) -> Option<NaiveDateTime> {
        self.suspended
    }
    
    /// Sets the card's suspended timestamp
    ///
    /// ### Arguments
    ///
    /// * `suspended` - The new suspended timestamp for the card
    pub fn set_suspended(&mut self, suspended: Option<DateTime<Utc>>) {
        self.suspended = suspended.map(|dt| dt.naive_utc());
    }

    /// Gets the card's sort position
    ///
    /// ### Returns
    ///
    /// The sort position of the card, or None if not set
    pub fn get_sort_position(&self) -> Option<f32> {
        self.sort_position
    }

    /// Sets the card's sort position
    ///
    /// ### Arguments
    ///
    /// * `sort_position` - The new sort position for the card
    pub fn set_sort_position(&mut self, sort_position: Option<f32>) {
        self.sort_position = sort_position;
    }

    /// Gets the card's priority offset
    ///
    /// ### Returns
    ///
    /// The priority offset of the card
    pub fn get_priority_offset(&self) -> f32 {
        self.priority_offset
    }

    /// Sets the card's priority offset
    ///
    /// ### Arguments
    ///
    /// * `priority_offset` - The new priority offset for the card
    pub fn set_priority_offset(&mut self, priority_offset: f32) {
        self.priority_offset = priority_offset;
    }

    /// Serializes the card to JSON with the priority offset folded into the priority field
    ///
    /// The returned JSON has:
    /// - `priority` = base priority + priority_offset, clamped to [0.0, 1.0]
    /// - `priority_offset` field removed
    ///
    /// ### Returns
    ///
    /// A serde_json::Value representing the card with effective priority
    pub fn to_json_hide_priority_offset(&self) -> serde_json::Value {
        let effective_priority = (self.priority + self.priority_offset).clamp(0.0, 1.0);
        let mut json = serde_json::to_value(self).expect("Card serialization should never fail");
        if let Some(obj) = json.as_object_mut() {
            obj.insert("priority".to_string(), serde_json::Value::from(effective_priority));
            obj.remove("priority_offset");
        }
        json
    }
}

#[cfg(test)]
mod prop_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_card_new() {
        let item_id = Uuid::new_v4().to_string();
        let card_index = 1;
        let next_review = Utc::now();
        
        let card = Card::new(item_id.clone(), card_index, next_review, 0.5);
        
        assert_eq!(card.get_item_id(), item_id);
        assert_eq!(card.get_card_index(), card_index);
        assert!(Uuid::parse_str(&card.get_id()).is_ok());
        assert_eq!(card.get_next_review(), next_review);
        assert_eq!(card.get_last_review(), None);
        assert_eq!(card.get_scheduler_data(), None);
    }
    
    #[test]
    fn test_new_card_sort_position_default() {
        let card = Card::new("item1".to_string(), 0, Utc::now(), 0.5);
        assert_eq!(card.get_sort_position(), None);
    }

    #[test]
    fn test_new_card_priority_offset_default() {
        let card = Card::new("item1".to_string(), 0, Utc::now(), 0.5);
        assert_eq!(card.get_priority_offset(), 0.0);
    }

    #[test]
    fn test_new_with_fields_sort_position_default() {
        let card = Card::new_with_fields(
            "id1".to_string(),
            "item1".to_string(),
            0,
            Utc::now(),
            None,
            None,
            0.5,
            None,
        );
        assert_eq!(card.get_sort_position(), None);
    }

    #[test]
    fn test_new_with_fields_priority_offset_default() {
        let card = Card::new_with_fields(
            "id1".to_string(),
            "item1".to_string(),
            0,
            Utc::now(),
            None,
            None,
            0.5,
            None,
        );
        assert_eq!(card.get_priority_offset(), 0.0);
    }

    #[test]
    fn test_to_json_hide_priority_offset_basic() {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), 0.5);
        card.set_priority_offset(0.03);
        let json = card.to_json_hide_priority_offset();
        let priority = json["priority"].as_f64().unwrap();
        assert!((priority - 0.53).abs() < 1e-5, "expected ~0.53, got {}", priority);
        assert!(json.get("priority_offset").is_none(), "priority_offset should be absent");
    }

    #[test]
    fn test_to_json_hide_priority_offset_clamps_low() {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), 0.0);
        card.set_priority_offset(-0.5);
        let json = card.to_json_hide_priority_offset();
        let priority = json["priority"].as_f64().unwrap();
        assert_eq!(priority, 0.0);
    }

    #[test]
    fn test_to_json_hide_priority_offset_clamps_high() {
        let mut card = Card::new("item1".to_string(), 0, Utc::now(), 1.0);
        card.set_priority_offset(0.5);
        let json = card.to_json_hide_priority_offset();
        let priority = json["priority"].as_f64().unwrap();
        assert_eq!(priority, 1.0);
    }

    #[test]
    fn test_card_scheduler_data() {
        let item_id = Uuid::new_v4().to_string();
        let card_index = 1;
        let next_review = Utc::now();
        let scheduler_data = Some(JsonValue(json!({
            "ease_factor": 2.5,
            "interval": 1,
            "repetitions": 0,
        })));
        
        let mut card = Card::new(item_id, card_index, next_review, 0.5);
        card.set_scheduler_data(scheduler_data.clone());
        
        assert_eq!(card.get_scheduler_data(), scheduler_data);
    }
} 
