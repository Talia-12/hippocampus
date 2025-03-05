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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::schema;
    use diesel::connection::SimpleConnection;
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
    
    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
    
    fn setup_test_db() -> DbPool {
        let database_url = ":memory:";
        let pool = db::init_pool(database_url);
        
        // Run migrations on the in-memory database
        let mut conn = pool.get().expect("Failed to get connection");
        conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
        conn.run_pending_migrations(MIGRATIONS).expect("Failed to run migrations");
        
        pool
    }
    
    #[test]
    fn test_create_item() {
        let pool = setup_test_db();
        let title = "Test Item".to_string();
        
        let result = create_item(&pool, title.clone());
        assert!(result.is_ok(), "Should create an item successfully");
        
        let item = result.unwrap();
        assert_eq!(item.title, title);
        assert!(!item.id.is_empty());
    }
    
    #[test]
    fn test_get_item() {
        let pool = setup_test_db();
        let title = "Test Item for Get".to_string();
        
        // First create an item
        let created_item = create_item(&pool, title.clone()).unwrap();
        
        // Then try to get it
        let result = get_item(&pool, &created_item.id);
        assert!(result.is_ok(), "Should get an item successfully");
        
        let item_option = result.unwrap();
        assert!(item_option.is_some(), "Item should exist");
        
        let item = item_option.unwrap();
        assert_eq!(item.id, created_item.id);
        assert_eq!(item.title, title);
    }
    
    #[test]
    fn test_get_nonexistent_item() {
        let pool = setup_test_db();
        
        // Try to get a non-existent item
        let result = get_item(&pool, "nonexistent-id");
        assert!(result.is_ok(), "Should not error for non-existent item");
        
        let item_option = result.unwrap();
        assert!(item_option.is_none(), "Item should not exist");
    }
    
    #[test]
    fn test_list_items() {
        let pool = setup_test_db();
        
        // Create a few items
        let titles = vec!["Item 1", "Item 2", "Item 3"];
        for title in &titles {
            create_item(&pool, title.to_string()).unwrap();
        }
        
        // List all items
        let result = list_items(&pool);
        assert!(result.is_ok(), "Should list items successfully");
        
        let items = result.unwrap();
        assert_eq!(items.len(), titles.len(), "Should have the correct number of items");
        
        // Check that all titles are present
        let item_titles: Vec<String> = items.iter().map(|item| item.title.clone()).collect();
        for title in titles {
            assert!(item_titles.contains(&title.to_string()), "Should contain title: {}", title);
        }
    }
    
    #[test]
    fn test_record_review() {
        let pool = setup_test_db();
        
        // First create an item
        let item = create_item(&pool, "Item to Review".to_string()).unwrap();
        
        // Record a review
        let rating = 2;
        let result = record_review(&pool, &item.id, rating);
        assert!(result.is_ok(), "Should record a review successfully");
        
        let review = result.unwrap();
        assert_eq!(review.item_id, item.id);
        assert_eq!(review.rating, rating);
        
        // Check that the item was updated with review information
        let updated_item = get_item(&pool, &item.id).unwrap().unwrap();
        assert!(updated_item.last_review.is_some(), "Last review should be set");
        assert!(updated_item.next_review.is_some(), "Next review should be set");
        
        // For rating 2, next review should be 3 days later
        let last_review = updated_item.last_review.unwrap();
        let next_review = updated_item.next_review.unwrap();
        let days_diff = (next_review.timestamp() - last_review.timestamp()) / (24 * 60 * 60);
        assert_eq!(days_diff, 3, "For rating 2, next review should be 3 days later");
    }
} 