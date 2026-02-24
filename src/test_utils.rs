use crate::*;
use proptest::prelude::*;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{DateTime, Utc};
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
    // Use a unique shared in-memory database for each test.
    // Plain ":memory:" gives each connection its own separate database,
    // so migrations run on one connection wouldn't be visible on others.
    // By using a unique URI with cache=shared, all connections in this pool
    // share the same in-memory database while remaining isolated from other tests.
    let unique_id = uuid::Uuid::new_v4();
    let database_url = format!("file:test_{}?mode=memory&cache=shared", unique_id);
    let pool = db::init_pool(&database_url);
    
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



/// Generates an arbitrary DateTime<Utc> within 2020-01-01 to 2030-01-01
pub fn arb_datetime_utc() -> impl Strategy<Value = DateTime<Utc>> {
    (1_577_836_800i64..1_893_456_000i64)
        .prop_map(|ts| DateTime::from_timestamp(ts, 0).unwrap())
}

/// Generates an optional arbitrary DateTime<Utc>
pub fn arb_optional_datetime_utc() -> impl Strategy<Value = Option<DateTime<Utc>>> {
    prop_oneof![
        Just(None),
        arb_datetime_utc().prop_map(Some),
    ]
}

/// Generates a valid priority value in [0.0, 1.0]
///
/// Uses integer-then-divide to ensure exact 0.0 and 1.0 are reachable
/// without floating point boundary issues.
pub fn arb_priority() -> impl Strategy<Value = f32> {
    (0u32..=1000u32).prop_map(|v| v as f32 / 1000.0)
}

/// Generates an invalid priority value outside [0.0, 1.0]
pub fn arb_invalid_priority() -> impl Strategy<Value = f32> {
    prop_oneof![
        (-1000.0f32..-0.001f32),
        (1.001f32..1000.0f32),
    ]
}

/// Generates an arbitrary SuspendedFilter variant
pub fn arb_suspended_filter() -> impl Strategy<Value = SuspendedFilter> {
    prop_oneof![
        Just(SuspendedFilter::Include),
        Just(SuspendedFilter::Exclude),
        Just(SuspendedFilter::Only),
    ]
}

/// Mutable card state fields for property testing
#[derive(Debug, Clone)]
pub struct CardMutations {
    pub next_review: DateTime<Utc>,
    pub last_review: Option<DateTime<Utc>>,
    pub priority: f32,
    pub suspended: Option<DateTime<Utc>>,
}

/// Generates arbitrary card mutation state
pub fn arb_card_mutations() -> impl Strategy<Value = CardMutations> {
    (
        arb_datetime_utc(),
        arb_optional_datetime_utc(),
        arb_priority(),
        arb_optional_datetime_utc(),
    ).prop_map(|(next_review, last_review, priority, suspended)| {
        CardMutations { next_review, last_review, priority, suspended }
    })
}

/// Generates an arbitrary sort position: 50% None, 50% Some(f32) in (-1000..1000)
pub fn arb_sort_position() -> impl Strategy<Value = Option<f32>> {
    prop_oneof![
        Just(None),
        (-1000.0f32..1000.0f32).prop_map(Some),
    ]
}

/// Generates a valid priority offset in [-0.05, +0.05] via integer division
pub fn arb_priority_offset() -> impl Strategy<Value = f32> {
    (-50i32..=50i32).prop_map(|v| v as f32 / 1000.0)
}

/// Generates a wide offset in [-2.0, 2.0] for clamping tests
pub fn arb_wide_offset() -> impl Strategy<Value = f32> {
    (-2000i32..=2000i32).prop_map(|v| v as f32 / 1000.0)
}

/// Generates any f32 value including NaN, ±Infinity, subnormals, etc.
pub fn arb_any_f32() -> impl Strategy<Value = f32> {
    proptest::num::f32::ANY
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
