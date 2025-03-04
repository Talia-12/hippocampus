use crate::schema::items;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Queryable, Insertable, AsChangeset, Debug, Serialize, Deserialize)]
#[diesel(table_name = items)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub next_review: Option<DateTime<Utc>>,
    pub last_review: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Item {
    pub fn new(title: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            title,
            next_review: None,
            last_review: None,
            created_at: now,
            updated_at: now,
        }
    }
} 