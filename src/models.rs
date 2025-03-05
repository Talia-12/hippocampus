use crate::schema::{items, reviews};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
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

#[derive(Queryable, Insertable, Debug, Serialize, Deserialize)]
#[diesel(table_name = reviews)]
pub struct Review {
    pub id: String,
    pub item_id: String,
    pub rating: i32,
    pub review_timestamp: DateTime<Utc>,
}

impl Review {
    pub fn new(item_id: &str, rating: i32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            item_id: item_id.to_string(),
            rating,
            review_timestamp: Utc::now(),
        }
    }
} 