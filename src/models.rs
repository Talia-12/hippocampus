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