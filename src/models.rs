/// Data models module
///
/// This module defines the core data structures used throughout the application.
/// It includes database models that map to database tables, as well as methods
/// for creating and manipulating these models.
use chrono::{DateTime, NaiveDateTime, Utc};
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
pub struct JsonValue(pub serde_json::Value);

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
#[derive(Queryable, Selectable, Insertable, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::item_types)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ItemType {
    /// Unique identifier for the item type (UUID v4 as string)
    id: String,
    
    /// The name of this item type
    name: String,
    
    /// When this item type was created
    created_at: NaiveDateTime,
}

/// Represents an item in the spaced repetition system
///
/// This struct maps directly to the `items` table in the database.
/// It contains all the information needed to track an item through the
/// spaced repetition review process, including review scheduling metadata.
#[derive(Queryable, Selectable, Insertable, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::items)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Item {
    /// Unique identifier for the item (UUID v4 as string)
    id: String,
    
    /// The type of this item
    item_type: String,
    
    /// The title of the item
    title: String,
    
    /// JSON data specific to this item type, stored as TEXT
    item_data: JsonValue,
    
    /// When this item was created
    created_at: NaiveDateTime,
    
    /// When this item was last updated
    updated_at: NaiveDateTime,
}

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

/// Represents a tag in the system
#[derive(Queryable, Selectable, Insertable, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::tags)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Tag {
    /// Unique identifier for the tag (UUID v4 as string)
    id: String,

    /// The name of the tag
    name: String,
    
    /// Whether the tag is visible to the user
    visible: bool,
    
    /// When this tag was created
    created_at: NaiveDateTime,
}


#[derive(Queryable, Selectable, Insertable, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::reviews)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Review { 
    /// Unique identifier for the review (UUID v4 as string)
    id: String,
    
    /// The ID of the card this review belongs to
    card_id: String,
    
    /// The rating given during this review
    rating: i32,
    
    /// When this review occurred
    review_timestamp: NaiveDateTime,
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
    
    /// Creates a new item type with all fields specified
    ///
    /// ### Arguments
    ///
    /// * `id` - The unique identifier for the item type
    /// * `name` - The name of the item type
    /// * `created_at` - When this item type was created
    ///
    /// ### Returns
    ///
    /// A new `ItemType` instance with the specified fields
    pub fn new_with_fields(id: String, name: String, created_at: DateTime<Utc>) -> Self {
        Self {
            id,
            name,
            created_at: created_at.naive_utc(),
        }
    }
    
    /// Gets the item type's ID
    ///
    /// ### Returns
    ///
    /// The unique identifier of the item type
    pub fn get_id(&self) -> String {
        self.id.clone()
    }
    
    /// Gets the item type's name
    ///
    /// ### Returns
    ///
    /// The name of the item type
    pub fn get_name(&self) -> String {
        self.name.clone()
    }
    
    /// Sets the item type's name
    ///
    /// ### Arguments
    ///
    /// * `name` - The new name for the item type
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }
    
    /// Gets the item type's creation timestamp
    ///
    /// ### Returns
    ///
    /// The timestamp when this item type was created
    pub fn get_created_at(&self) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(self.created_at, Utc)
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
    
    /// Creates a new item with all fields specified
    ///
    /// ### Arguments
    ///
    /// * `id` - The unique identifier for the item
    /// * `item_type` - The type of item to create
    /// * `title` - The title of the item
    /// * `data` - The data associated with the item
    /// * `created_at` - When this item was created
    /// * `updated_at` - When this item was last updated
    ///
    /// ### Returns
    ///
    /// A new `Item` instance with the specified fields
    pub fn new_with_fields(
        id: String, 
        item_type: String, 
        title: String, 
        data: JsonValue, 
        created_at: DateTime<Utc>, 
        updated_at: DateTime<Utc>
    ) -> Self {
        Self {
            id,
            item_type,
            title,
            item_data: data,
            created_at: created_at.naive_utc(),
            updated_at: updated_at.naive_utc(),
        }
    }
    
    /// Gets the item's ID
    ///
    /// ### Returns
    ///
    /// The unique identifier of the item
    pub fn get_id(&self) -> String {
        self.id.clone()
    }
    
    /// Gets the item's type
    ///
    /// ### Returns
    ///
    /// The type of the item
    pub fn get_item_type(&self) -> String {
        self.item_type.clone()
    }
    
    /// Sets the item's type
    ///
    /// ### Arguments
    ///
    /// * `item_type` - The new type for the item
    pub fn set_item_type(&mut self, item_type: String) {
        self.item_type = item_type;
        self.updated_at = Utc::now().naive_utc();
    }
    
    /// Gets the item's title
    ///
    /// ### Returns
    ///
    /// The title of the item
    pub fn get_title(&self) -> String {
        self.title.clone()
    }
    
    /// Sets the item's title
    ///
    /// ### Arguments
    ///
    /// * `title` - The new title for the item
    pub fn set_title(&mut self, title: String) {
        self.title = title;
        self.updated_at = Utc::now().naive_utc();
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
    
    /// Gets the item's creation timestamp
    ///
    /// ### Returns
    ///
    /// The timestamp when this item was created
    pub fn get_created_at(&self) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(self.created_at, Utc)
    }
    
    /// Gets the item's last update timestamp
    ///
    /// ### Returns
    ///
    /// The timestamp when this item was last updated
    pub fn get_updated_at(&self) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(self.updated_at, Utc)
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
    
    /// Gets the item ID this card belongs to
    ///
    /// ### Returns
    ///
    /// The ID of the item this card belongs to
    pub fn get_item_id(&self) -> String {
        self.item_id.clone()
    }
    
    /// Sets the item ID this card belongs to
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
    
    /// Gets the card's next review timestamp
    ///
    /// ### Returns
    ///
    /// The timestamp when this card should next be reviewed
    pub fn get_next_review(&self) -> Option<DateTime<Utc>> {
        self.next_review.map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
    }
    
    /// Sets the card's next review timestamp
    ///
    /// ### Arguments
    ///
    /// * `next_review` - The new next review timestamp for the card
    pub fn set_next_review(&mut self, next_review: Option<DateTime<Utc>>) {
        self.next_review = next_review.map(|dt| dt.naive_utc());
    }
    
    /// Gets the card's last review timestamp
    ///
    /// ### Returns
    ///
    /// The timestamp when this card was last reviewed
    pub fn get_last_review(&self) -> Option<DateTime<Utc>> {
        self.last_review.map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc))
    }
    
    /// Sets the card's last review timestamp
    ///
    /// ### Arguments
    ///
    /// * `last_review` - The new last review timestamp for the card
    pub fn set_last_review(&mut self, last_review: Option<DateTime<Utc>>) {
        self.last_review = last_review.map(|dt| dt.naive_utc());
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


impl Tag {
    /// Creates a new tag
    /// 
    /// ### Arguments
    ///
    /// * `name` - The name of the tag
    /// * `visible` - Whether the tag is visible to the user
    ///
    /// ### Returns
    /// 
    /// A new `Tag` instance with:
    /// - A randomly generated UUID
    /// - The provided name
    /// - The provided visibility
    /// - Creation timestamp set to the current time
    pub fn new(name: String, visible: bool) -> Self {
        Self {
            id: Uuid::new_v4().to_string(), 
            name,
            visible,
            created_at: Utc::now().naive_utc(),
        }
    }
    
    /// Creates a new tag with all fields specified
    ///
    /// ### Arguments
    ///
    /// * `id` - The unique identifier for the tag
    /// * `name` - The name of the tag
    /// * `visible` - Whether the tag is visible to the user
    /// * `created_at` - When this tag was created
    ///
    /// ### Returns
    ///
    /// A new `Tag` instance with the specified fields
    pub fn new_with_fields(
        id: String,
        name: String,
        visible: bool,
        created_at: DateTime<Utc>
    ) -> Self {
        Self {
            id,
            name,
            visible,
            created_at: created_at.naive_utc(),
        }
    }
    
    /// Gets the tag's ID
    ///
    /// ### Returns
    ///
    /// The unique identifier of the tag
    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    /// Gets the tag's name
    ///
    /// ### Returns
    ///
    /// The name of the tag
    pub fn get_name(&self) -> String {
        self.name.clone()
    }
    
    /// Sets the tag's name
    ///
    /// ### Arguments
    ///
    /// * `name` - The new name for the tag
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    /// Gets the tag's visibility
    ///
    /// ### Returns
    ///
    /// The visibility of the tag
    pub fn get_visible(&self) -> bool {
        self.visible
    }

    /// Sets the tag's visibility
    ///
    /// ### Arguments
    ///
    /// * `visible` - The new visibility of the tag
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Gets the tag's creation timestamp
    ///
    /// ### Returns
    ///
    /// The timestamp when this tag was created
    pub fn get_created_at(&self) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(self.created_at, Utc)
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
    
    /// Creates a new review with all fields specified
    ///
    /// ### Arguments
    ///
    /// * `id` - The unique identifier for the review
    /// * `card_id` - The ID of the card being reviewed
    /// * `rating` - The rating given during the review
    /// * `review_timestamp` - When this review occurred
    ///
    /// ### Returns
    ///
    /// A new `Review` instance with the specified fields
    pub fn new_with_fields(
        id: String,
        card_id: String,
        rating: i32,
        review_timestamp: DateTime<Utc>
    ) -> Self {
        Self {
            id,
            card_id,
            rating,
            review_timestamp: review_timestamp.naive_utc(),
        }
    }
    
    /// Gets the review's ID
    ///
    /// ### Returns
    ///
    /// The unique identifier of the review
    pub fn get_id(&self) -> String {
        self.id.clone()
    }
    
    /// Gets the review timestamp
    ///
    /// ### Returns
    ///
    /// The timestamp when this review occurred
    pub fn get_review_timestamp(&self) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(self.review_timestamp, Utc)
    }
    
    /// Sets the review timestamp
    ///
    /// ### Arguments
    ///
    /// * `review_timestamp` - The new timestamp for the review
    pub fn set_review_timestamp(&mut self, review_timestamp: DateTime<Utc>) {
        self.review_timestamp = review_timestamp.naive_utc();
    }

    /// Gets the ID of the card this review belongs to
    ///
    /// ### Returns
    ///
    /// The ID of the card this review belongs to
    pub fn get_card_id(&self) -> String {
        self.card_id.clone()
    }
    
    /// Sets the ID of the card this review belongs to
    ///
    /// ### Arguments
    ///
    /// * `card_id` - The new card ID for the review
    pub fn set_card_id(&mut self, card_id: String) {
        self.card_id = card_id;
    }

    /// Gets the rating given during this review
    ///
    /// ### Returns
    ///
    /// The rating given during this review
    pub fn get_rating(&self) -> i32 {
        self.rating
    }
    
    /// Sets the rating given during this review
    ///
    /// ### Arguments
    ///
    /// * `rating` - The new rating for the review
    pub fn set_rating(&mut self, rating: i32) {
        self.rating = rating;
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

    /// Tests the creation of a new tag
    /// 
    /// This test verifies that a new tag can be created with the correct name and visibility.
    /// It also checks that the creation timestamp is set correctly.
    #[test]
    fn test_tag_new() { 
        let name = "Test Tag".to_string();
        let visible = true;
        
        let tag = Tag::new(name.clone());
        
        assert_eq!(tag.name, name);
        assert_eq!(tag.visible, visible);
        assert_eq!(tag.id.len(), 36);
        assert!(tag.created_at <= Utc::now().naive_utc());
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