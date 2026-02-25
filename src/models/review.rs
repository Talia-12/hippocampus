use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

impl Review {
    /// Creates a new review for a card
    ///
    /// ### Arguments
    ///
    /// * `card_id` - The ID of the card being reviewed
    /// * `rating` - The rating given during the review
    ///
    /// ### Returns
    ///
    /// A new `Review` instance with the specified card ID and rating
    ///
    /// ### Panics
    ///
    /// This function will panic if the rating is not in the range 1-3.
    pub fn new(card_id: &str, rating: i32) -> Self {
        // Validate the rating
        if rating < 1 || rating > 4 {
            panic!("Rating must be between 1 and 4, got {}", rating);
        }
        
        Self {
            id: Uuid::new_v4().to_string(),
            card_id: card_id.to_string(),
            rating,
            review_timestamp: Utc::now().naive_utc(),
        }
    }
    
    /// Creates a new review with all fields specified
    ///
    /// ### Arguments
    ///
    /// * `id` - The unique identifier for the review
    /// * `card_id` - The ID of the card this review belongs to
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
    
    /// Gets the review's timestamp as a DateTime<Utc>
    ///
    /// ### Returns
    ///
    /// The timestamp when this review occurred
    pub fn get_review_timestamp(&self) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(self.review_timestamp, Utc)
    }
    
    /// Gets the review's raw timestamp
    ///
    /// ### Returns
    ///
    /// The raw NaiveDateTime when this review occurred
    pub fn get_review_timestamp_raw(&self) -> NaiveDateTime {
        self.review_timestamp
    }
    
    /// Sets the review's timestamp
    ///
    /// ### Arguments
    ///
    /// * `review_timestamp` - The new timestamp for the review
    pub fn set_review_timestamp(&mut self, review_timestamp: DateTime<Utc>) {
        self.review_timestamp = review_timestamp.naive_utc();
    }
    
    /// Gets the review's card ID
    ///
    /// ### Returns
    ///
    /// The ID of the card this review belongs to
    pub fn get_card_id(&self) -> String {
        self.card_id.clone()
    }
    
    /// Sets the review's card ID
    ///
    /// ### Arguments
    ///
    /// * `card_id` - The new card ID for the review
    pub fn set_card_id(&mut self, card_id: String) {
        self.card_id = card_id;
    }
    
    /// Gets the review's rating
    ///
    /// ### Returns
    ///
    /// The rating given during this review
    pub fn get_rating(&self) -> i32 {
        self.rating
    }
    
    /// Sets the review's rating
    ///
    /// ### Arguments
    ///
    /// * `rating` - The new rating for the review
    pub fn set_rating(&mut self, rating: i32) {
        self.rating = rating;
    }
}

#[cfg(test)]
mod prop_tests;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_review_new() {
        let card_id = Uuid::new_v4().to_string();
        let rating = 2;
        
        let review = Review::new(&card_id, rating);
        
        assert_eq!(review.get_card_id(), card_id);
        assert_eq!(review.get_rating(), rating);
        assert!(Uuid::parse_str(&review.get_id()).is_ok());
        
        // Ensure review_timestamp is within the last second
        let now = Utc::now();
        let timestamp = review.get_review_timestamp();
        let diff = now.signed_duration_since(timestamp);
        
        assert!(diff.num_seconds() < 1);
    }
    
    #[test]
    #[should_panic(expected = "Rating must be between 1 and 4")]
    fn test_review_invalid_rating() {
        let card_id = Uuid::new_v4().to_string();
        let invalid_rating = 0;
        
        // This should panic
        let _ = Review::new(&card_id, invalid_rating);
    }
} 
