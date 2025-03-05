use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Queryable, Selectable, Insertable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::items)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]

pub struct Item {
    pub id: String,
    pub title: String,
    pub next_review: Option<NaiveDateTime>,
    pub last_review: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl Item {
    pub fn new(title: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            title,
            next_review: None,
            last_review: None,
            created_at: now.naive_utc(),
            updated_at: now.naive_utc(),
        }
    }
    
    // Helper methods to convert between DateTime and i64
    pub fn get_created_at(&self) -> NaiveDateTime {
        self.created_at
    }
    
    pub fn get_updated_at(&self) -> NaiveDateTime {
        self.updated_at
    }
    
    pub fn get_next_review(&self) -> Option<NaiveDateTime> {
        self.next_review
    }
    
    pub fn get_last_review(&self) -> Option<NaiveDateTime> {
        self.last_review
    }
}


pub struct DieselItem {
    pub id: String,
    pub title: String,
    pub next_review: Option<NaiveDateTime>,
    pub last_review: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}


#[derive(Queryable, Selectable, Insertable, Debug, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::reviews)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Review {
    pub id: String,
    pub item_id: String,
    pub rating: i32,
    pub review_timestamp: NaiveDateTime,
}

impl Review {
    pub fn new(item_id: &str, rating: i32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            item_id: item_id.to_string(),
            rating,
            review_timestamp: Utc::now().naive_utc(),
        }
    }
    
    // Helper method to get DateTime from timestamp
    pub fn get_review_timestamp(&self) -> NaiveDateTime {
        self.review_timestamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    
    #[test]
    fn test_item_new() {
        let title = "Test Item".to_string();
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
    
    #[test]
    fn test_item_getters() {
        let title = "Test Item".to_string();
        let item = Item::new(title);
        
        // Test getter methods
        assert_eq!(item.get_created_at(), item.created_at);
        assert_eq!(item.get_updated_at(), item.updated_at);
        assert_eq!(item.get_next_review(), None);
        assert_eq!(item.get_last_review(), None);
        
        // Create an item with review dates
        let mut item_with_reviews = Item::new("Item with reviews".to_string());
        let now = Utc::now().naive_utc();
        let next_week = now + Duration::days(7);
        
        item_with_reviews.last_review = Some(now);
        item_with_reviews.next_review = Some(next_week);
        
        assert_eq!(item_with_reviews.get_last_review(), Some(now));
        assert_eq!(item_with_reviews.get_next_review(), Some(next_week));
    }
    
    #[test]
    fn test_review_new() {
        let item_id = "test-item-id";
        let rating = 3;
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