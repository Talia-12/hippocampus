use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a tag in the system
#[derive(Queryable, Selectable, Insertable, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[diesel(table_name = crate::schema::tags)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Tag {
    /// Unique identifier for the tag (UUID v4 as string)
    id: String,

    /// The name of the tag
    name: String,
        
    /// When this tag was created
    created_at: NaiveDateTime,

    /// Whether the tag is visible to the user
    visible: bool,
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
    /// A new `Tag` instance with the specified name and visibility
    pub fn new(name: String, visible: bool) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            created_at: Utc::now().naive_utc(),
            visible,
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
            created_at: created_at.naive_utc(),
            visible,
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
    /// Whether the tag is visible to the user
    pub fn get_visible(&self) -> bool {
        self.visible
    }
    
    /// Sets the tag's visibility
    ///
    /// ### Arguments
    ///
    /// * `visible` - The new visibility for the tag
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
    
    /// Gets the tag's creation timestamp as a DateTime<Utc>
    ///
    /// ### Returns
    ///
    /// The timestamp when this tag was created
    pub fn get_created_at(&self) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(self.created_at, Utc)
    }
    
    /// Gets the tag's raw creation timestamp
    ///
    /// ### Returns
    ///
    /// The raw NaiveDateTime when this tag was created
    pub fn get_created_at_raw(&self) -> NaiveDateTime {
        self.created_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tag_new() { 
        let name = "Important".to_string();
        let visible = true;
        
        let tag = Tag::new(name.clone(), visible);
        
        assert_eq!(tag.get_name(), name);
        assert_eq!(tag.get_visible(), visible);
        assert!(Uuid::parse_str(&tag.get_id()).is_ok());
        
        // Ensure created_at is within the last second
        let now = Utc::now();
        let created_at = tag.get_created_at();
        let diff = now.signed_duration_since(created_at);
        
        assert!(diff.num_seconds() < 1);
    }
} 