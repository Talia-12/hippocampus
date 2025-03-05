use crate::db::DbPool;
use crate::models::{Item, Review};
use crate::schema::{items, reviews};
use diesel::prelude::*;
use anyhow::Result;
use chrono::Utc;
use chrono::Duration;

pub fn create_item(pool: &DbPool, new_title: String) -> Result<Item> {
    let conn = &mut pool.get()?;
    let new_item = Item::new(new_title);
    diesel::insert_into(items::table)
        .values(&new_item)
        .execute(conn)?;
    Ok(new_item)
}

pub fn get_item(pool: &DbPool, item_id: &str) -> Result<Option<Item>> {
    let conn = &mut pool.get()?;
    let result = items::table
        .filter(items::id.eq(item_id))
        .first::<Item>(conn)
        .optional()?;
    Ok(result)
}

pub fn list_items(pool: &DbPool) -> Result<Vec<Item>> {
    let conn = &mut pool.get()?;
    let result = items::table.load::<Item>(conn)?;
    Ok(result)
}

pub fn record_review(pool: &DbPool, item_id_val: &str, rating_val: i32) -> Result<Review> {
    let conn = &mut pool.get()?;
    
    // 1) Insert the review record
    let new_review = Review::new(item_id_val, rating_val);
    diesel::insert_into(reviews::table)
        .values(&new_review)
        .execute(conn)?;

    // 2) Retrieve the item and update next_review
    let mut item = items::table
        .filter(items::id.eq(item_id_val))
        .first::<Item>(conn)?;
    
    let now = Utc::now();
    item.last_review = Some(now.naive_utc());
    
    // Simple spaced repetition logic
    let days_to_add = match rating_val {
        1 => 1,  // If difficult, review tomorrow
        2 => 3,  // If medium, review in 3 days
        3 => 7,  // If easy, review in a week
        _ => 1,  // Default to tomorrow
    };
    
    item.next_review = Some(now.naive_utc() + Duration::days(days_to_add));
    item.updated_at = now.naive_utc();
    
    diesel::update(items::table.filter(items::id.eq(item_id_val)))
        .set((
            items::next_review.eq(item.next_review),
            items::last_review.eq(item.last_review),
            items::updated_at.eq(item.updated_at),
        ))
        .execute(conn)?;
    
    Ok(new_review)
} 