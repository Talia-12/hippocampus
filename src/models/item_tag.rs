use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

/// Represents an association between an item and a tag
#[derive(Queryable, Selectable, Insertable, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::item_tags)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ItemTag {
    /// The ID of the item
    item_id: String,
    
    /// The ID of the tag
    tag_id: String,

    /// When this item tag was created
    created_at: NaiveDateTime,
}

impl ItemTag {
    /// Creates a new item tag association
    ///
    /// ### Arguments
    ///
    /// * `item_id` - The ID of the item
    /// * `tag_id` - The ID of the tag
    ///
    /// ### Returns
    ///
    /// A new `ItemTag` instance with the specified item ID and tag ID
    pub fn new(item_id: String, tag_id: String) -> Self {
        Self {
            item_id,
            tag_id,
            created_at: Utc::now().naive_utc(),
        }
    }
    
    /// Gets the item ID
    ///
    /// ### Returns
    ///
    /// The ID of the item in this association
    pub fn get_item_id(&self) -> String {
        self.item_id.clone()
    }
    
    /// Gets the tag ID
    ///
    /// ### Returns
    ///
    /// The ID of the tag in this association
    pub fn get_tag_id(&self) -> String {
        self.tag_id.clone()
    }
    
    /// Gets the creation timestamp as a DateTime<Utc>
    ///
    /// ### Returns
    ///
    /// The timestamp when this association was created
    pub fn get_created_at(&self) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(self.created_at, Utc)
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod prop_tests; 