use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Data transfer object for creating a new item
///
/// This struct is used to deserialize JSON requests for creating items.
#[derive(Serialize, Deserialize, Debug)]
pub struct CreateItemDto {
    /// The item type ID
    pub item_type_id: String,
    
    /// The title or content of the item to be remembered
    pub title: String,
    
    /// Additional data specific to the item type
    pub item_data: serde_json::Value,

    /// The priority of the item, between 0 and 1
    #[serde(default = "default_priority")]
    pub priority: f32,
}


/// Data transfer object for updating an item
///
/// This struct is used to deserialize JSON requests for updating items.
#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateItemDto {
    /// The new title or content of the item to be remembered
    pub title: Option<String>,
    
    /// The new additional data specific to the item type
    pub item_data: Option<serde_json::Value>,
}


/// The default priority for an item
fn default_priority() -> f32 { 0.5 }


/// Data transfer object for creating a new review
///
/// This struct is used to deserialize JSON requests for recording reviews.
#[derive(Serialize, Deserialize, Debug)]
pub struct CreateReviewDto {
    /// The ID of the card being reviewed
    pub card_id: String,
    
    /// The rating given during the review (typically 1-3)
    pub rating: i32,
}

/// Data transfer object for creating a new item type
///
/// This struct is used to deserialize JSON requests for creating item types.
#[derive(Serialize, Deserialize, Debug)]
pub struct CreateItemTypeDto {
    /// The name of the item type
    pub name: String,
}

/// Data transfer object for creating a new card
///
/// This struct is used to deserialize JSON requests for creating cards.
#[derive(Serialize, Deserialize, Debug)]
pub struct CreateCardDto {
    /// The index of the card within its item
    pub card_index: i32,

    /// The priority of the card, between 0 and 1
    pub priority: f32,
}

/// Data transfer object for creating a new tag
///
/// This struct is used to deserialize JSON requests for creating tags.
#[derive(Serialize, Deserialize, Debug)]
pub struct CreateTagDto {
    /// The name of the tag
    pub name: String,
    
    /// The visibility of the tag
    pub visible: bool,
}


#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum SuspendedFilter {
    Include,
    #[default]
    Exclude,
    Only,
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

    /// The minimum last review date to filter by
    pub last_review_after: Option<DateTime<Utc>>,

    /// Whether to include suspended cards
    pub suspended_filter: SuspendedFilter,

    /// The minimum suspended date to filter by
    pub suspended_after: Option<DateTime<Utc>>,

    /// The maximum suspended date to filter by
    pub suspended_before: Option<DateTime<Utc>>,
}

/// Builder for GetQueryDto
pub struct GetQueryDtoBuilder {
    item_type_id: Option<String>,
    tag_ids: Vec<String>,
    next_review_before: Option<DateTime<Utc>>,
    last_review_after: Option<DateTime<Utc>>,
    suspended_filter: SuspendedFilter,
    suspended_after: Option<DateTime<Utc>>,
    suspended_before: Option<DateTime<Utc>>,
}

impl GetQueryDtoBuilder {
    /// Creates a new GetQueryDtoBuilder
    pub fn new() -> Self {
        Self {
            item_type_id: None,
            tag_ids: Vec::new(),
            next_review_before: None,
            last_review_after: None,
            suspended_filter: SuspendedFilter::default(),
            suspended_after: None,
            suspended_before: None,
        }
    }

    /// Sets the item type ID to filter by
    pub fn item_type_id(mut self, item_type_id: String) -> Self {
        self.item_type_id = Some(item_type_id);
        self
    }

    /// Sets the tag IDs to filter by
    pub fn tag_ids(mut self, tag_ids: Vec<String>) -> Self {
        self.tag_ids = tag_ids;
        self
    }

    /// Adds a tag ID to the filter
    pub fn add_tag_id(mut self, tag_id: String) -> Self {
        self.tag_ids.push(tag_id);
        self
    }

    /// Sets the maximum next review date to filter by
    pub fn next_review_before(mut self, next_review_before: DateTime<Utc>) -> Self {
        self.next_review_before = Some(next_review_before);
        self
    }

    /// Sets the minimum last review date to filter by
    pub fn last_review_after(mut self, last_review_after: DateTime<Utc>) -> Self {
        self.last_review_after = Some(last_review_after);
        self
    }

    /// Sets the suspended filter
    pub fn suspended_filter(mut self, suspended_filter: SuspendedFilter) -> Self {
        self.suspended_filter = suspended_filter;
        self
    }

    /// Sets the minimum suspended date to filter by
    pub fn suspended_after(mut self, suspended_after: DateTime<Utc>) -> Self {
        self.suspended_after = Some(suspended_after);
        self
    }

    /// Sets the maximum suspended date to filter by
    pub fn suspended_before(mut self, suspended_before: DateTime<Utc>) -> Self {
        self.suspended_before = Some(suspended_before);
        self
    }

    /// Builds the GetQueryDto
    pub fn build(self) -> GetQueryDto {
        GetQueryDto {
            item_type_id: self.item_type_id,
            tag_ids: self.tag_ids,
            next_review_before: self.next_review_before,
            last_review_after: self.last_review_after,
            suspended_filter: self.suspended_filter,
            suspended_after: self.suspended_after,
            suspended_before: self.suspended_before,
        }
    }
}


use std::fmt;

impl fmt::Display for GetQueryDto {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GetQueryDto {{ ")?;
        
        if let Some(item_type) = &self.item_type_id {
            write!(f, "item_type_id: {}, ", item_type)?;
        } else {
            write!(f, "item_type_id: None, ")?;
        }
        
        write!(f, "tag_ids: [")?;
        for (i, tag_id) in self.tag_ids.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", tag_id)?;
        }
        write!(f, "], ")?;
        
        if let Some(review_date) = self.next_review_before {
            write!(f, "next_review_before: {} ", review_date)?;
        } else {
            write!(f, "next_review_before: None ")?;
        }
        
        if let Some(review_date) = self.last_review_after {
            write!(f, "last_review_after: {} ", review_date)?;
        } else {
            write!(f, "last_review_after: None ")?;
        }

        write!(f, "}}")
    }
}
