use crate::db::DbPool;
use crate::models::Card;
use diesel::prelude::*;

/// Sets up a test database connection pool for testing
pub fn setup_test_db() -> DbPool {
    use diesel::sqlite::SqliteConnection;
    use diesel_migrations::MigrationHarness;
    use crate::db;
    
    // Create a new in-memory database connection for testing
    let mut conn = SqliteConnection::establish(":memory:")
        .expect("Failed to create in-memory database for testing");
    
    // Run migrations to create the schema
    crate::run_migrations(&mut conn);
    
    // Create a connection pool with just this one connection
    db::DbPool::new(conn)
}

/// Updates a card in the database
///
/// This is a helper function for tests that need to update a card
/// directly (e.g., for setting next_review date)
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `card` - The card to update
///
/// ### Returns
///
/// A Result indicating success or failure
pub fn update_card(pool: &DbPool, card: &Card) -> Result<(), diesel::result::Error> {
    use crate::schema::cards::dsl::*;
    
    let conn = &mut pool.get().unwrap();
    
    diesel::update(cards.find(card.get_id()))
        .set((
            next_review.eq(card.get_next_review_raw()),
            last_review.eq(card.get_last_review_raw()),
            scheduler_data.eq(card.get_scheduler_data()),
        ))
        .execute(conn)?;
    
    Ok(())
} 