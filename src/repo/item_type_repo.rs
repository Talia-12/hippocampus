use crate::db::DbPool;
use crate::models::ItemType;
use diesel::prelude::*;
use anyhow::Result;

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
pub fn create_item_type(pool: &DbPool, name: String) -> Result<ItemType> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Create a new item type with the provided name
    let new_item_type = ItemType::new(name);
    
    // Insert the new item type into the database
    diesel::insert_into(crate::schema::item_types::table)
        .values(&new_item_type)
        .execute(conn)?;
    
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
pub fn get_item_type(pool: &DbPool, id: &str) -> Result<Option<ItemType>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for the item type with the specified ID
    let result = crate::schema::item_types::table
        .find(id)
        .first::<ItemType>(conn)
        .optional()?;
    
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
pub fn list_item_types(pool: &DbPool) -> Result<Vec<ItemType>> {
    // Get a connection from the pool
    let conn = &mut pool.get()?;
    
    // Query the database for all item types
    let result = crate::schema::item_types::table
        .load::<ItemType>(conn)?;
    
    // Return the list of item types
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::tests::setup_test_db;
    
    #[test]
    fn test_create_item_type() {
        let pool = setup_test_db();
        let name = "Vocabulary".to_string();
        
        let item_type = create_item_type(&pool, name.clone()).unwrap();
        
        assert_eq!(item_type.get_name(), name);
    }
    
    #[test]
    fn test_get_item_type() {
        let pool = setup_test_db();
        let name = "Vocabulary".to_string();
        
        let created_item_type = create_item_type(&pool, name.clone()).unwrap();
        let retrieved_item_type = get_item_type(&pool, &created_item_type.get_id()).unwrap().unwrap();
        
        assert_eq!(retrieved_item_type.get_name(), name);
        assert_eq!(retrieved_item_type.get_id(), created_item_type.get_id());
    }
    
    #[test]
    fn test_list_item_types() {
        let pool = setup_test_db();
        
        // Create some item types
        let item_type1 = create_item_type(&pool, "Vocabulary".to_string()).unwrap();
        let item_type2 = create_item_type(&pool, "Grammar".to_string()).unwrap();
        
        // List all item types
        let item_types = list_item_types(&pool).unwrap();
        
        // Verify that the list contains the created item types
        assert_eq!(item_types.len(), 2);
        assert!(item_types.iter().any(|it| it.get_id() == item_type1.get_id()));
        assert!(item_types.iter().any(|it| it.get_id() == item_type2.get_id()));
    }
} 