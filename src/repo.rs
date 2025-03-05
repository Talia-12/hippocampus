use crate::db::DbPool;
use crate::models::{Item, Review};
use crate::schema::items::dsl::*;
use crate::schema::reviews::dsl::*;
use diesel::prelude::*;
use anyhow::Result;
use chrono::Utc;
use chrono::Duration;

pub fn create_item(pool: &DbPool, new_title: String) -> Result<Item> {
    let conn = &mut pool.get()?;
    let new_item = Item::new(new_title);
    diesel::insert_into(crate::schema::items::table)
        .values(&new_item)
        .execute(conn)?;
    Ok(new_item)
}

pub fn get_item(pool: &DbPool, item_id: &str) -> Result<Option<Item>> {
    let conn = &mut pool.get()?;
    let result = crate::schema::items::table
        .filter(crate::schema::items::id.eq(item_id))
        .first::<Item>(conn)
        .optional()?;
    Ok(result)
}

pub fn list_items(pool: &DbPool) -> Result<Vec<Item>> {
    let conn = &mut pool.get()?;
    let result = crate::schema::items::table.load::<Item>(conn)?;
    Ok(result)
}

pub fn record_review(pool: &DbPool, item_id_val: &str, rating_val: i32) -> Result<Review> {
    let conn = &mut pool.get()?;
    
    // 1) Insert the review record
    let new_review = Review::new(item_id_val, rating_val);
    diesel::insert_into(crate::schema::reviews::table)
        .values(&new_review)
        .execute(conn)?;

    // 2) Retrieve the item and update next_review
    let mut item = crate::schema::items::table
        .filter(crate::schema::items::id.eq(item_id_val))
        .first::<Item>(conn)?;

    let now = Utc::now();
    item.last_review = Some(now);
    
    // Simple scheduling example:
    // rating 1 => add 1 day, rating 2 => 3 days, rating 3 => 7 days
    let add_days = match rating_val {
        1 => 1,
        2 => 3,
        3 => 7,
        _ => 1,
    };
    
    item.next_review = Some(now + Duration::days(add_days));
    item.updated_at = now;

    diesel::update(crate::schema::items::table.filter(crate::schema::items::id.eq(item_id_val)))
        .set((
            crate::schema::items::next_review.eq(item.next_review),
            crate::schema::items::last_review.eq(item.last_review),
            crate::schema::items::updated_at.eq(item.updated_at),
        ))
        .execute(conn)?;

    Ok(new_review)
} 