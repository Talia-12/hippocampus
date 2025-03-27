use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Data transfer object for creating a new item
///
/// This struct is used to deserialize JSON requests for creating items.
#[derive(Deserialize, Debug)]
pub struct CreateItemDto {
    /// The item type ID
    pub item_type_id: String,
    
    /// The title or content of the item to be remembered
    pub title: String,
    
    /// Additional data specific to the item type
    pub item_data: serde_json::Value,
}

/// Data transfer object for creating a new review
///
/// This struct is used to deserialize JSON requests for recording reviews.
#[derive(Deserialize, Debug)]
pub struct CreateReviewDto {
    /// The ID of the card being reviewed
    pub card_id: String,
    
    /// The rating given during the review (typically 1-3)
    pub rating: i32,
}

/// Data transfer object for creating a new item type
///
/// This struct is used to deserialize JSON requests for creating item types.
#[derive(Deserialize, Debug)]
pub struct CreateItemTypeDto {
    /// The name of the item type
    pub name: String,
}

/// Data transfer object for creating a new card
///
/// This struct is used to deserialize JSON requests for creating cards.
#[derive(Deserialize, Debug)]
pub struct CreateCardDto {
    /// The index of the card within its item
    pub card_index: i32,

    /// The priority of the card, between 0 and 1
    pub priority: f32,
}

/// Data transfer object for creating a new tag
///
/// This struct is used to deserialize JSON requests for creating tags.
#[derive(Deserialize, Debug)]
pub struct CreateTagDto {
    /// The name of the tag
    pub name: String,
    
    /// The visibility of the tag
    pub visible: bool,
}

/// Data transfer object for getting all items or cards matching a query
/// 
/// This struct is used to deserialize JSON requests for getting all items or cards matching a query.
#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct GetQueryDto {
    /// The ID of the item type to filter by
    pub item_type_id: Option<String>,
    
    /// The IDs of the tags to filter by
    pub tag_ids: Vec<String>,
    
    /// The maximum next review date to filter by
    pub next_review_before: Option<DateTime<Utc>>,
}

/// Data transfer object for updating a card's priority
///
/// This struct is used to deserialize JSON requests for updating a card's priority.
#[derive(Deserialize, Debug)]
pub struct UpdateCardPriorityDto {
    /// The new priority for the card, between 0 and 1
    pub priority: f32,
}