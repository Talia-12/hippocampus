use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

    /// The review function used for scheduling cards of this item type
    review_function: String,
}

impl ItemType {
    /// Creates a new item type
    ///
    /// ### Arguments
    ///
    /// * `name` - The name of the item type
    /// * `review_function` - The review function used for scheduling (e.g. "fsrs", "incremental_queue")
    pub fn new(name: String, review_function: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            created_at: Utc::now().naive_utc(),
            review_function,
        }
    }
    
    /// Creates a new item type with all fields specified
    ///
    /// ### Arguments
    ///
    /// * `id` - The unique identifier for the item type
    /// * `name` - The name of the item type
    /// * `created_at` - When this item type was created
    /// * `review_function` - The review function used for scheduling
    ///
    /// ### Returns
    ///
    /// A new `ItemType` instance with the specified fields
    pub fn new_with_fields(id: String, name: String, created_at: DateTime<Utc>, review_function: String) -> Self {
        Self {
            id,
            name,
            created_at: created_at.naive_utc(),
            review_function,
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
    
    /// Gets the item type's creation timestamp as a DateTime<Utc>
    ///
    /// ### Returns
    ///
    /// The timestamp when this item type was created
    pub fn get_created_at(&self) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(self.created_at, Utc)
    }
    
    /// Gets the item type's raw creation timestamp
    ///
    /// ### Returns
    ///
    /// The raw NaiveDateTime when this item type was created
    pub fn get_created_at_raw(&self) -> NaiveDateTime {
        self.created_at
    }

    /// Gets the item type's review function
    ///
    /// ### Returns
    ///
    /// The review function used for scheduling cards of this item type
    pub fn get_review_function(&self) -> String {
        self.review_function.clone()
    }

    /// Sets the item type's review function
    ///
    /// ### Arguments
    ///
    /// * `review_function` - The new review function for the item type
    pub fn set_review_function(&mut self, review_function: String) {
        self.review_function = review_function;
    }
}

#[cfg(test)]
mod prop_tests;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_item_type_new() {
        let name = "Type 1".to_string();
        let item_type = ItemType::new(name.clone(), "fsrs".to_string());

        assert_eq!(item_type.get_name(), name);
        assert_eq!(item_type.get_review_function(), "fsrs");
        assert!(Uuid::parse_str(&item_type.get_id()).is_ok());

        // Ensure created_at is within the last second
        let now = Utc::now();
        let created_at = item_type.get_created_at();
        let diff = now.signed_duration_since(created_at);

        assert!(diff.num_seconds() < 1);
    }
} 