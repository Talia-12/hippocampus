use crate::db::DbPool;
use crate::models::Item;
use crate::schema::items::dsl::*;
use diesel::prelude::*;
use anyhow::Result;

pub fn create_item(pool: &DbPool, new_title: String) -> Result<Item> {
    let conn = &mut pool.get()?;
    let new_item = Item::new(new_title);
    diesel::insert_into(items)
        .values(&new_item)
        .execute(conn)?;
    Ok(new_item)
}

pub fn get_item(pool: &DbPool, item_id: &str) -> Result<Option<Item>> {
    let conn = &mut pool.get()?;
    let result = items
        .filter(id.eq(item_id))
        .first::<Item>(conn)
        .optional()?;
    Ok(result)
}

pub fn list_items(pool: &DbPool) -> Result<Vec<Item>> {
    let conn = &mut pool.get()?;
    let result = items.load::<Item>(conn)?;
    Ok(result)
} 