/// Data models module
///
/// This module defines the core data structures used throughout the application.
/// It includes database models that map to database tables, as well as methods
/// for creating and manipulating these models.
use chrono::{NaiveDateTime, Utc};
use diesel::deserialize::{FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::{prelude::*, serialize};
use diesel::serialize::{Output, ToSql, IsNull};
use diesel::sql_types::Text;
use diesel::sqlite::{Sqlite, SqliteValue};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a JSON value in the database
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[diesel(sql_type = Text)]
struct JsonValue(serde_json::Value);

impl FromSql<Text, Sqlite> for JsonValue {
    fn from_sql(value: SqliteValue<'_, '_, '_>) -> diesel::deserialize::Result<Self> {
        let text = <String as FromSql<Text, Sqlite>>::from_sql(value)?;
        let value = serde_json::from_str(&text)?;
        Ok(JsonValue(value))
    }
}

impl ToSql<Text, Sqlite> for JsonValue {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
        out.set_value(serde_json::to_string(&self.0)?);
        Ok(IsNull::No)
    }
}


/// Represents an item type in the system
#[derive(Queryable, Selectable, Insertable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::item_types)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ItemType {
    /// Unique identifier for the item type (UUID v4 as string)
    pub id: String,
    
    /// The name of this item type
    pub name: String,
    
    /// When this item type was created
    pub created_at: NaiveDateTime,
}

/// Represents an item in the spaced repetition system
///
/// This struct maps directly to the `items` table in the database.
/// It contains all the information needed to track an item through the
/// spaced repetition review process, including review scheduling metadata.
#[derive(Queryable, Selectable, Insertable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::items)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Item {
    /// Unique identifier for the item (UUID v4 as string)
    pub id: String,
    
    /// The type of this item
    pub item_type: String,
    
    /// The title of the item
    pub title: String,
    
    /// JSON data specific to this item type, stored as TEXT
    pub item_data: JsonValue,
    
    /// When this item was created
    pub created_at: NaiveDateTime,
    
    /// When this item was last updated
    pub updated_at: NaiveDateTime,
}

/// Represents a card in the spaced repetition system
#[derive(Queryable, Selectable, Insertable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::cards)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Card {
    /// Unique identifier for the card (UUID v4 as string)
    pub id: String,
    
    /// The ID of the item this card belongs to
    pub item_id: String,
    
    /// The index of this card within its item
    pub card_index: i32,
    
    /// When this card should next be reviewed
    pub next_review: Option<NaiveDateTime>,
    
    /// When this card was last reviewed
    pub last_review: Option<NaiveDateTime>,
    
    /// JSON data for the scheduler, stored as TEXT
    pub scheduler_data: Option<JsonValue>,
}

#[derive(Queryable, Selectable, Insertable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::reviews)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Review { 
    /// Unique identifier for the review (UUID v4 as string)
    pub id: String,
    
    /// The ID of the card this review belongs to
    pub card_id: String,
    
    /// The rating given during this review
    pub rating: i32,
    
    /// When this review occurred
    pub review_timestamp: NaiveDateTime,
}

impl ItemType {
    /// Creates a new item type
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            created_at: Utc::now().naive_utc(),
        }
    }
}

impl Item {
    /// Creates a new item
    ///
    /// ### Arguments
    ///
    /// * `item_type` - The type of item to create
    /// * `title` - The title of the item
    /// * `data` - The data associated with the item
    ///
    /// ### Returns
    ///
    /// A new `Item` instance with:
    /// - A randomly generated UUID
    /// - The provided item_type, title, and data
    pub fn new(item_type: String, title: String, data: JsonValue) -> Self {
        let now = Utc::now().naive_utc();
        
        Self {
            id: Uuid::new_v4().to_string(),
            item_type,
            title,
            item_data: data,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// Gets the item's data as a JSON value
    ///
    /// ### Returns
    ///
    /// The data associated with the item
    pub fn get_data(&self) -> JsonValue {
        self.item_data.clone()
    }
    
    /// Sets the item's data from a JSON value
    ///
    /// ### Arguments
    ///
    /// * `data` - The new data to set for the item
    pub fn set_data(&mut self, data: JsonValue) {
        self.item_data = data;
        self.updated_at = Utc::now().naive_utc();
    }
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
    /// A new `Card` instance with:
    /// - A randomly generated UUID
    /// - The provided item_id and card_index
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
    
    /// Gets the card's scheduler data as a JSON value
    ///
    /// ### Returns
    ///
    /// The scheduler data for the card
    pub fn get_scheduler_data(&self) -> Option<JsonValue> {
        self.scheduler_data.clone()
    }

    /// Sets the card's scheduler data from a JSON value
    ///
    /// ### Arguments
    ///
    /// * `data` - The new scheduler data to set for the card
    pub fn set_scheduler_data(&mut self, data: Option<JsonValue>) {
        self.scheduler_data = data;
    }
}

impl Review {
    /// Creates a new review for an item
    ///
    /// ### Arguments
    ///
    /// * `card_id` - The ID of the card being reviewed
    /// * `rating` - The rating given during the review (typically 1-3)
    ///
    /// ### Returns
    ///
    /// A new `Review` instance with:
    /// - A randomly generated UUID
    /// - The provided item_id and rating
    /// - Current timestamp for review_timestamp
    pub fn new(card_id: &str, rating: i32) -> Self {
        Self {
            // Generate a new random UUID v4 and convert to string
            id: Uuid::new_v4().to_string(),
            
            // Store the ID of the item being reviewed
            card_id: card_id.to_string(),
            
            // Store the rating provided by the user
            rating,
            
            // Set the review timestamp to the current time
            review_timestamp: Utc::now().naive_utc(),
        }
    }
    
    /// Gets the review timestamp
    ///
    /// ### Returns
    ///
    /// The timestamp when this review occurred
    pub fn get_review_timestamp(&self) -> NaiveDateTime {
        self.review_timestamp
    }

    /// Gets the ID of the card this review belongs to
    ///
    /// ### Returns
    ///
    /// The ID of the card this review belongs to
    pub fn get_card_id(&self) -> String {
        self.card_id.clone()
    }

    /// Gets the rating given during this review
    ///
    /// ### Returns
    ///
    /// The rating given during this review
    pub fn get_rating(&self) -> i32 {
        self.rating
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests the creation of a new item type
    /// 
    /// This test verifies that a new item type can be created with the correct name and ID.
    /// It also checks that the creation timestamp is set correctly.
    #[test]
    fn test_item_type_new() {
        let name = "Test Type".to_string();
        let item_type = ItemType::new(name.clone());
        
        assert_eq!(item_type.name, name);
        assert_eq!(item_type.id.len(), 36);
        assert!(item_type.created_at <= Utc::now().naive_utc());
    }

    /// Tests the creation of a new item
    /// 
    /// This test verifies that a new item can be created with the correct item type, title, and data.
    /// It also checks that the creation and update timestamps are set correctly.
    #[test]
    fn test_item_new() {
        let item_type = "test-type".to_string();
        let title = "Test Item".to_string();
        let data = serde_json::json!({
            "key": "value",
            "number": 42
        });
        
        let item = Item::new(item_type.clone(), title.clone(), JsonValue(data.clone()));
        
        assert_eq!(item.item_type, item_type);
        assert_eq!(item.title, title);
        assert_eq!(item.get_data(), JsonValue(data));
        assert_eq!(item.id.len(), 36);
        assert!(item.created_at <= Utc::now().naive_utc());
        assert!(item.updated_at <= Utc::now().naive_utc());
    }

    /// Tests the creation of a new card
    /// 
    /// This test verifies that a new card can be created with the correct item ID and card index.
    /// It also checks that the card has no next or last review timestamps, and no scheduler data.
    #[test]
    fn test_card_new() {
        let item_id = "test-item-id".to_string();
        let card_index = 0;
        
        let card = Card::new(item_id.clone(), card_index);
        
        assert_eq!(card.item_id, item_id);
        assert_eq!(card.card_index, card_index);
        assert_eq!(card.id.len(), 36);
        assert!(card.next_review.is_none());
        assert!(card.last_review.is_none());
        assert!(card.scheduler_data.is_none());
    }

    /// Tests the creation of a new card
    /// 
    /// This test verifies that the scheduler data can be set and retrieved correctly.
    #[test]
    fn test_card_scheduler_data() {
        let mut card = Card::new("test-item-id".to_string(), 0);
        
        let data = JsonValue(serde_json::json!({
            "interval": 86400,
            "ease_factor": 2.5
        }));
        
        card.set_scheduler_data(Some(data.clone()));
        assert_eq!(card.get_scheduler_data(), Some(data));
        
        card.set_scheduler_data(None);
        assert!(card.get_scheduler_data().is_none());
        }

    /// Tests the creation of a new review
    /// 
    /// This test verifies that a new review can be created with the correct card ID and rating.
    /// It also checks that the review timestamp is set correctly.
    #[test]
    fn test_review_new() {
        let card_id = "test-card-id".to_string();
        let rating = 3;
        
        let review = Review::new(&card_id, rating); 
    
        assert_eq!(review.card_id, card_id);
        assert_eq!(review.rating, rating);
        assert_eq!(review.id.len(), 36);
        assert!(review.review_timestamp <= Utc::now().naive_utc());
    }    
} 