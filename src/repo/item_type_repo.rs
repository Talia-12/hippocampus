use crate::db::{DbPool, ExecuteWithRetry};
use crate::models::ItemType;
use diesel::prelude::*;
use anyhow::Result;
use tracing::{instrument, debug, info};

/// Creates a new item type in the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `name` - The name for the new item type
///
/// ### Returns
///
/// A Result containing the newly created ItemType if successful
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database insert operation fails
#[instrument(skip(pool), fields(name = %name))]
pub async fn create_item_type(pool: &DbPool, name: String) -> Result<ItemType> {
    debug!("Creating new item type");
    
    // Get a connection from the pool
    let conn = &mut pool.get()?;

    // #[cfg(test)] {
    //     use tracing::warn;

    //     if !name.contains("Test") {
    //         warn!("Item type name should normally contain 'Test' for testing purposes");
    //     }
    // }

    // Create a new item type with the provided name
    let new_item_type = ItemType::new(name);
    
    // Insert the new item type into the database
    diesel::insert_into(crate::schema::item_types::table)
        .values(new_item_type.clone())
        .execute_with_retry(conn).await?;
    
    info!("Successfully created item type with id: {}", new_item_type.get_id());
    
    // Return the newly created item type
    Ok(new_item_type)
}


/// Retrieves an item type from the database by its ID
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
/// * `id` - The ID of the item type to retrieve
///
/// ### Returns
///
/// A Result containing an Option with the ItemType if found, or None if not found
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails for reasons other than the item type not existing
#[instrument(skip(pool), fields(item_type_id = %id))]
pub fn get_item_type(pool: &DbPool, id: &str) -> Result<Option<ItemType>> {
    debug!("Retrieving item type");
    
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for the item type with the specified ID
    let result = crate::schema::item_types::table
        .find(id)
        .first::<ItemType>(conn)
        .optional()?;
    
    if result.is_some() {
        debug!("Item type found");
    } else {
        debug!("Item type not found");
    }
    
    // Return the item type if found, or None if not found
    Ok(result)
}


/// Retrieves all item types from the database
///
/// ### Arguments
///
/// * `pool` - A reference to the database connection pool
///
/// ### Returns
///
/// A Result containing a vector of all ItemTypes in the database
///
/// ### Errors
///
/// Returns an error if:
/// - Unable to get a connection from the pool
/// - The database query fails
#[instrument(skip(pool))]
pub fn list_item_types(pool: &DbPool) -> Result<Vec<ItemType>> {
    debug!("Listing all item types");
    
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for all item types
    let result = crate::schema::item_types::table
        .load::<ItemType>(conn)?;
    
    info!("Retrieved {} item types", result.len());
    
    // Return the list of item types
    Ok(result)
}


#[cfg(test)]
mod tests;
#[cfg(test)]
mod prop_tests;