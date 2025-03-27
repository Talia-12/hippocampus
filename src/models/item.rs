use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::JsonValue;

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

impl Item {
    /// Creates a new item with the given title
    ///
    /// This method automatically generates a UUID v4 for the ID and sets
    /// the created_at and updated_at timestamps to the current time.
    ///
    /// ### Arguments
    ///
    /// * `title` - The title of the item
    ///
    /// ### Returns
    ///
    /// A new `Item` instance with the specified title
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
    /// This method is primarily used for testing and database deserialization.
    ///
    /// ### Arguments
    ///
    /// * `id` - The unique identifier for the item
    /// * `item_type` - The type of the item
    /// * `title` - The title of the item
    /// * `data` - JSON data for the item
    /// * `created_at` - When the item was created
    /// * `updated_at` - When the item was last updated
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
    
    /// Gets the item's data
    ///
    /// ### Returns
    ///
    /// The JSON data of the item
    pub fn get_data(&self) -> JsonValue {
        self.item_data.clone()
    }
    
    /// Sets the item's data
    ///
    /// ### Arguments
    ///
    /// * `data` - The new JSON data for the item
    pub fn set_data(&mut self, data: JsonValue) {
        self.item_data = data;
        self.updated_at = Utc::now().naive_utc();
    }
    
    /// Gets the item's creation timestamp as a DateTime<Utc>
    ///
    /// ### Returns
    ///
    /// The timestamp when this item was created
    pub fn get_created_at(&self) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(self.created_at, Utc)
    }
    
    /// Gets the item's raw creation timestamp
    ///
    /// ### Returns
    ///
    /// The raw NaiveDateTime when this item was created
    pub fn get_created_at_raw(&self) -> NaiveDateTime {
        self.created_at
    }
    
    /// Gets the item's updated timestamp as a DateTime<Utc>
    ///
    /// ### Returns
    ///
    /// The timestamp when this item was last updated
    pub fn get_updated_at(&self) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(self.updated_at, Utc)
    }
    
    /// Gets the item's raw updated timestamp
    ///
    /// ### Returns
    ///
    /// The raw NaiveDateTime when this item was last updated
    pub fn get_updated_at_raw(&self) -> NaiveDateTime {
        self.updated_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_item_new() {
        let item_type = "vocabulary".to_string();
        let title = "Example Item".to_string();
        let data = JsonValue(json!({
            "front": "Hello",
            "back": "World"
        }));
        
        let item = Item::new(item_type.clone(), title.clone(), data.clone());
        
        assert_eq!(item.get_title(), title);
        assert_eq!(item.get_item_type(), item_type);
        assert_eq!(item.get_data().0, data.0);
        assert!(Uuid::parse_str(&item.get_id()).is_ok());
        
        // Ensure created_at and updated_at are within the last second
        let now = Utc::now();
        let created_at = item.get_created_at();
        let updated_at = item.get_updated_at();
        let diff1 = now.signed_duration_since(created_at);
        let diff2 = now.signed_duration_since(updated_at);
        
        assert!(diff1.num_seconds() < 1);
        assert!(diff2.num_seconds() < 1);
    }
} 