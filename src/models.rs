/// Data models module
///
/// This module defines the core data structures used throughout the application.
/// It includes database models that map to database tables, as well as methods
/// for creating and manipulating these models.
use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    
    /// The title or content of the item to be remembered
    pub title: String,
    
    /// When this item should next be reviewed (scheduled review time)
    pub next_review: Option<NaiveDateTime>,
    
    /// When this item was last reviewed
    pub last_review: Option<NaiveDateTime>,
    
    /// When this item was created
    pub created_at: NaiveDateTime,
    
    /// When this item was last updated
    pub updated_at: NaiveDateTime,
}

impl Item {
    /// Creates a new item with the given title
    ///
    /// ### Arguments
    ///
    /// * `title` - The title or content of the item to be remembered
    ///
    /// ### Returns
    ///
    /// A new `Item` instance with:
    /// - A randomly generated UUID
    /// - The provided title
    /// - No review history (next_review and last_review are None)
    /// - Current timestamp for created_at and updated_at
    pub fn new(title: String) -> Self {
        // Get the current time to use for timestamps
        let now = Utc::now();
        
        Self {
            // Generate a new random UUID v4 and convert to string
            id: Uuid::new_v4().to_string(),
            
            // Use the provided title
            title,
            
            // New items have no review history
            next_review: None,
            last_review: None,
            
            // Set creation and update timestamps to current time
            created_at: now.naive_utc(),
            updated_at: now.naive_utc(),
        }
    }
    
    /// Gets the item's creation timestamp
    ///
    /// ### Returns
    ///
    /// The timestamp when this item was created
    pub fn get_created_at(&self) -> NaiveDateTime {
        self.created_at
    }
    
    /// Gets the item's last update timestamp
    ///
    /// ### Returns
    ///
    /// The timestamp when this item was last updated
    pub fn get_updated_at(&self) -> NaiveDateTime {
        self.updated_at
    }
    
    /// Gets the item's next scheduled review time
    ///
    /// ### Returns
    ///
    /// An Option containing the timestamp for the next review,
    /// or None if the item has never been reviewed
    pub fn get_next_review(&self) -> Option<NaiveDateTime> {
        self.next_review
    }
    
    /// Gets the item's last review timestamp
    ///
    /// ### Returns
    ///
    /// An Option containing the timestamp of the last review,
    /// or None if the item has never been reviewed
    pub fn get_last_review(&self) -> Option<NaiveDateTime> {
        self.last_review
    }
}

/// Represents a review record in the spaced repetition system
///
/// This struct maps directly to the `reviews` table in the database.
/// It tracks individual review events, including the user's rating of
/// how well they remembered the item.
#[derive(Queryable, Selectable, Insertable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::reviews)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Review {
    /// Unique identifier for the review (UUID v4 as string)
    pub id: String,
    
    /// The ID of the item that was reviewed
    pub item_id: String,
    
    /// The rating given during the review (typically 1-3, where higher is better)
    pub rating: i32,
    
    /// When this review occurred
    pub review_timestamp: NaiveDateTime,
}

impl Review {
    /// Creates a new review for an item
    ///
    /// ### Arguments
    ///
    /// * `item_id` - The ID of the item being reviewed
    /// * `rating` - The rating given during the review (typically 1-3)
    ///
    /// ### Returns
    ///
    /// A new `Review` instance with:
    /// - A randomly generated UUID
    /// - The provided item_id and rating
    /// - Current timestamp for review_timestamp
    pub fn new(item_id: &str, rating: i32) -> Self {
        Self {
            // Generate a new random UUID v4 and convert to string
            id: Uuid::new_v4().to_string(),
            
            // Store the ID of the item being reviewed
            item_id: item_id.to_string(),
            
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    
    /// Tests the creation of a new item
    ///
    /// This test verifies that:
    /// 1. The item is created with the correct title
    /// 2. A valid UUID is generated
    /// 3. Timestamps are set correctly
    /// 4. Review fields are initially None
    #[test]
    fn test_item_new() {
        // Create a test title
        let title = "Test Item".to_string();
        
        // Create a new item with the test title
        let item = Item::new(title.clone());
        
        // Check that the item was created with the correct title
        assert_eq!(item.title, title);
        
        // Check that the UUID is valid
        assert!(!item.id.is_empty());
        assert_eq!(item.id.len(), 36); // UUID v4 string length
        
        // Check that the timestamps are set
        assert!(item.created_at <= Utc::now().naive_utc());
        assert!(item.updated_at <= Utc::now().naive_utc());
        
        // Check that review fields are None
        assert!(item.next_review.is_none());
        assert!(item.last_review.is_none());
    }
    
    /// Tests the getter methods for Item properties
    ///
    /// This test verifies that:
    /// 1. The getter methods return the correct values
    /// 2. The getters work for both None and Some values
    #[test]
    fn test_item_getters() {
        // Create a test item
        let title = "Test Item".to_string();
        let item = Item::new(title);
        
        // Test getter methods for a new item
        assert_eq!(item.get_created_at(), item.created_at);
        assert_eq!(item.get_updated_at(), item.updated_at);
        assert_eq!(item.get_next_review(), None);
        assert_eq!(item.get_last_review(), None);
        
        // Create an item with review dates
        let mut item_with_reviews = Item::new("Item with reviews".to_string());
        let now = Utc::now().naive_utc();
        let next_week = now + Duration::days(7);
        
        // Set review dates
        item_with_reviews.last_review = Some(now);
        item_with_reviews.next_review = Some(next_week);
        
        // Test getter methods for an item with review dates
        assert_eq!(item_with_reviews.get_last_review(), Some(now));
        assert_eq!(item_with_reviews.get_next_review(), Some(next_week));
    }
    
    /// Tests the creation of a new review
    ///
    /// This test verifies that:
    /// 1. The review is created with the correct item_id and rating
    /// 2. A valid UUID is generated
    /// 3. The timestamp is set correctly
    /// 4. The getter method returns the correct timestamp
    #[test]
    fn test_review_new() {
        // Create test data
        let item_id = "test-item-id";
        let rating = 3;
        
        // Create a new review
        let review = Review::new(item_id, rating);
        
        // Check that the review was created with the correct item_id and rating
        assert_eq!(review.item_id, item_id);
        assert_eq!(review.rating, rating);
        
        // Check that the UUID is valid
        assert!(!review.id.is_empty());
        assert_eq!(review.id.len(), 36); // UUID v4 string length
        
        // Check that the timestamp is set
        assert!(review.review_timestamp <= Utc::now().naive_utc());
        
        // Test getter method
        assert_eq!(review.get_review_timestamp(), review.review_timestamp);
    }
} 