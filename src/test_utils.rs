use crate::*;
use proptest::prelude::*;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use diesel::connection::SimpleConnection;
use diesel::RunQueryDsl;
use serde_json::{Value, Number};
use std::sync::Arc;
use tower::ServiceExt;

/// Sets up a test database with migrations applied
///
/// This function:
/// 1. Creates an in-memory SQLite database
/// 2. Enables foreign key constraints
/// 3. Runs all migrations to set up the schema
///
/// ### Returns
///
/// An Arc-wrapped database connection pool connected to the in-memory database
pub fn setup_test_db() -> Arc<db::DbPool> {
    // Use an in-memory database for testing
    let database_url = ":memory:";
    let pool = db::init_pool(database_url);
    
    // Get a connection from the pool
    let mut conn = pool.get().expect("Failed to get connection");
    
    // Enable foreign key constraints for SQLite
    conn.batch_execute("PRAGMA foreign_keys = ON").unwrap();
    
    // Run all migrations to set up the schema
    run_migrations(&mut conn);
    
    // Wrap the pool in an Arc for thread-safe sharing
    Arc::new(pool)
}


use diesel::sql_types::Text;
use diesel::QueryableByName;

#[derive(QueryableByName, Debug)]
struct TableName {
    #[diesel(sql_type = Text)]
    name: String,
}

/// Tests the setup_test_db function
///
/// This test verifies that:
/// 1. The test database can be created and connected to
/// 2. The database has the expected tables
/// 3. The database can be queried successfully
#[tokio::test]
async fn test_setup_test_db() {
    let pool = setup_test_db();
    assert!(pool.get().is_ok());

    // Check that all migrations were run, i.e. the tables were created
    let mut conn = pool.get().unwrap();
    let result = diesel::sql_query("SELECT name FROM sqlite_master WHERE type='table'")
        .execute(&mut conn);
    assert!(result.is_ok());
    
    println!("Result: {:?}", result);

    // Get the names of the tables
    let table_names: Vec<TableName> = diesel::sql_query("SELECT name FROM sqlite_master WHERE type='table'")
        .load(&mut conn)
        .expect("Failed to load table names");
    
    println!("Tables: {:?}", table_names);
    
    // Verify that we have the expected tables
    assert!(table_names.len() > 0, "No tables found in the database");

    // test interacting with each of the found tables
    let expected_tables = vec![
        "cards", "item_tags", "item_types", "items", "reviews", "tags", 
        "__diesel_schema_migrations" // Diesel's migration tracking table
    ];
    
    for table in expected_tables {
        let exists = table_names.iter().any(|t| t.name == table);
        assert!(exists, "Table '{}' not found in database", table);
        
        // Test a simple query on each table
        let query = format!("SELECT COUNT(*) FROM {}", table);
        let result = diesel::sql_query(&query).execute(&mut conn);
        assert!(result.is_ok(), "Failed to query table '{}': {:?}", table, result.err());
        
        println!("Table '{}' exists and is queryable", table);
    }

    drop(conn);

    // test interacting with the app
    let app = create_app(pool.clone());

    // test interacting with the item_types table
    let request = Request::builder()
        .uri("/item_types")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    // send the request to the app
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "Response status is not OK (err: {:?})", axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap());
}



pub fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<f64>()
            .prop_map(Number::from_f64)
            .prop_filter("f64s must be parseable as numbers", |v| v.is_some())
            .prop_map(|s| Value::Number(s.unwrap())),
        ".*".prop_map(Value::String)
    ];

    leaf.prop_recursive(
        8, // 8 levels deep
        256, // maximum size of 256 nodes
        10, // We put up to 10 items per collection
        |inner| prop_oneof![
            // Take the inner strategy and make the two recursive cases.
            prop::collection::vec(inner.clone(), 0..10)
                .prop_map(Value::Array),
            prop::collection::hash_map(".*", inner, 0..10)
                .prop_map(serde_json::to_value)
                .prop_filter("hashmap to map must succeed", |v| v.is_ok())
                .prop_map(|s| s.unwrap()),
        ])
}
