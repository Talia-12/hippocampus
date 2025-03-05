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

} 