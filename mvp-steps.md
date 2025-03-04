Below is an example “MVP roadmap” for your LLM agent to follow when bootstrapping the project. It focuses on the absolute essentials for a minimal spaced repetition server, ensuring that you can start testing core functionality early. The steps are intentionally broken down into small pieces—roughly the size of commits a developer might make when they “commit early, commit often.” Adjust as you see fit!

---

## 1. Decide the MVP Scope

1. **Single Item Type**: For now, support only a simple “flashcard” type. No type registry, no fancy item schemas.
2. **Basic Schema**: 
   - `items` table (UUID primary key, title, next_review, last_review, created_at, updated_at).
   - `reviews` table (UUID primary key, item_id, rating, review_timestamp).
3. **Minimal Scheduling**: Use a simple formula to set `next_review` (e.g., rating 3 → +1 day). No FSRS yet.
4. **Minimal API** (REST to keep it straightforward):
   - `POST /items` to create an item
   - `GET /items/:id` to fetch an item
   - `POST /reviews` to record a review (which updates `next_review`)
   - `GET /items` to list items (optional filter: items due)
5. **Testing**: 
   - Unit tests for CRUD and the simple scheduling logic
   - Integration tests for the REST endpoints

This is enough to let you write tests, confirm data is being stored and retrieved correctly, and demonstrate the spaced repetition logic on a single item type.

---

## 2. Project Setup & Directory Structure

### Step A: Create the Cargo Project
1. **Initialize a new Rust binary crate**:  
   ```bash
   cargo new srs-server --bin
   cd srs-server
   ```
2. **Set up a Git repo** (if you haven’t already):
   ```bash
   git init
   git add .
   git commit -m "Initial project scaffold"
   ```

### Step B: Add Dependencies
1. Open `Cargo.toml` and add the following to `[dependencies]` (exact versions can be the latest semver-compatible):
   ```toml
   [dependencies]
   # Web framework (Axum or Warp; example uses Axum)
   axum = "0.6"
   tokio = { version = "1.28", features = ["macros"] }
   hyper = "0.14"  # for lower-level HTTP support if needed

   # Database
   diesel = { version = "2.1.0", features = ["sqlite"] }
   diesel_migrations = "2.0.0"
   r2d2 = "0.8"               # Optional for connection pooling
   r2d2_diesel = "1.0"        # Diesel-specific pooling

   # Serde for serialization
   serde = { version = "1.0", features = ["derive"] }
   serde_json = "1.0"

   # UUID generation
   uuid = "1.3"

   # Time management
   chrono = { version = "0.4", features = ["serde"] }

   # Error handling
   anyhow = "1.0"
   thiserror = "1.0"

   # For logging
   tracing = "0.1"
   tracing-subscriber = "0.3"

   # For testing
   tokio-test = "0.4"
   reqwest = { version = "0.11", features = ["json"] }
   ```
2. Commit your changes:
   ```bash
   git add Cargo.toml
   git commit -m "Add Axum, Diesel, Serde, and other dependencies"
   ```

---

## 3. Database and Migrations

### Step C: Configure Diesel & Database Connection
1. **Create a `.env` file** (not committed to source, or .env.example if you prefer):
   ```bash
   echo "DATABASE_URL=srs_server.db" > .env
   ```
2. **Install Diesel CLI** (if not installed):
   ```bash
   cargo install diesel_cli --no-default-features --features sqlite
   ```
3. **Set up the initial migration folder**:
   ```bash
   diesel setup
   ```
   This will create a `migrations` directory and an empty database file `srs_server.db`.

4. **Create a new migration** for your `items` table:
   ```bash
   diesel migration generate create_items
   ```
5. **Edit** the generated `migrations/<timestamp>_create_items/up.sql`:
   ```sql
   CREATE TABLE items (
       id TEXT PRIMARY KEY,
       title TEXT NOT NULL,
       next_review DATETIME,
       last_review DATETIME,
       created_at DATETIME NOT NULL,
       updated_at DATETIME NOT NULL
   );
   ```
   Then in the `down.sql`:
   ```sql
   DROP TABLE items;
   ```
6. **Apply the migration**:
   ```bash
   diesel migration run
   ```
7. **Commit**:
   ```bash
   git add .
   git commit -m "Set up Diesel and create items table migration"
   ```

### Step D: Implement `diesel` Setup in Code
1. In `src/main.rs` (or a dedicated `src/db.rs`), set up:
   ```rust
   // src/db.rs
   use diesel::prelude::*;
   use diesel::sqlite::SqliteConnection;
   use r2d2::{ Pool, PooledConnection };
   use r2d2_diesel::ConnectionManager;

   pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;
   
   pub fn init_pool(database_url: &str) -> DbPool {
       let manager = ConnectionManager::<SqliteConnection>::new(database_url);
       Pool::builder().build(manager).expect("Failed to create pool.")
   }
   ```
2. In `src/main.rs`, load `.env`, initialize the pool, run migrations on startup if you want:
   ```rust
   use dotenv::dotenv;
   use std::env;

   fn main() {
       dotenv().ok();
       let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
       let pool = db::init_pool(&database_url);
       // ... start the Axum server, pass the pool as a shared state ...
   }
   ```
3. Commit:
   ```bash
   git add src/db.rs src/main.rs
   git commit -m "Add Diesel pool initialization"
   ```

---

## 4. Models and Basic CRUD

### Step E: Define Item Model & Diesel Schema
1. **Run `diesel print-schema`** to generate Rust table definitions:
   ```bash
   diesel print-schema > src/schema.rs
   ```
2. **Create an `Item` struct** in `src/models.rs` that matches the `items` table:
   ```rust
   // src/models.rs
   use crate::schema::items;
   use chrono::{DateTime, Utc};
   use uuid::Uuid;

   #[derive(Queryable, Insertable, AsChangeset, Debug)]
   #[diesel(table_name = items)]
   pub struct Item {
       pub id: String,
       pub title: String,
       pub next_review: Option<DateTime<Utc>>,
       pub last_review: Option<DateTime<Utc>>,
       pub created_at: DateTime<Utc>,
       pub updated_at: DateTime<Utc>,
   }

   impl Item {
       pub fn new(title: String) -> Self {
           let now = chrono::Utc::now();
           Self {
               id: Uuid::new_v4().to_string(),
               title,
               next_review: None,
               last_review: None,
               created_at: now,
               updated_at: now,
           }
       }
   }
   ```
3. Commit:
   ```bash
   git add src/schema.rs src/models.rs
   git commit -m "Add Diesel schema and Item model"
   ```

### Step F: Implement Basic CRUD
1. **Create a repository-like module** in `src/repo.rs`:
   ```rust
   // src/repo.rs
   use crate::db::DbPool;
   use crate::models::Item;
   use crate::schema::items::dsl::*;
   use diesel::prelude::*;
   use anyhow::Result;

   pub fn create_item(pool: &DbPool, new_title: String) -> Result<Item> {
       let conn = &mut pool.get()?;
       let new_item = Item::new(new_title);
       diesel::insert_into(items).values(&new_item).execute(conn)?;
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
   ```
   (You can add update/delete similarly if needed for the MVP.)
2. **Commit**:
   ```bash
   git add src/repo.rs
   git commit -m "Implement basic CRUD for Item"
   ```

---

## 5. Minimal API (Axum)

### Step G: Create Axum Handlers
1. **Set up routes** in `src/main.rs` (or a dedicated `src/routes.rs`):
   ```rust
   use axum::{
       routing::{get, post},
       Router, Json, extract::State,
   };
   use std::sync::Arc;
   use serde::Deserialize;

   #[derive(Deserialize)]
   struct CreateItemDto {
       title: String,
   }

   async fn create_item_handler(
       State(pool): State<DbPool>,
       Json(payload): Json<CreateItemDto>,
   ) -> Json<Item> {
       let item = repo::create_item(&pool, payload.title)
           .expect("Failed to create item");
       Json(item)
   }

   async fn get_item_handler(
       State(pool): State<DbPool>,
       axum::extract::Path(item_id): axum::extract::Path<String>,
   ) -> Json<Option<Item>> {
       let item = repo::get_item(&pool, &item_id)
           .expect("Failed to retrieve item");
       Json(item)
   }

   async fn list_items_handler(
       State(pool): State<DbPool>,
   ) -> Json<Vec<Item>> {
       let all_items = repo::list_items(&pool)
           .expect("Failed to list items");
       Json(all_items)
   }

   fn build_app(pool: DbPool) -> Router {
       Router::new()
           .route("/items", post(create_item_handler).get(list_items_handler))
           .route("/items/:id", get(get_item_handler))
           .with_state(pool)
   }
   ```
2. **In `main`, launch the Axum server**:
   ```rust
   #[tokio::main]
   async fn main() {
       dotenv().ok();
       let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
       let pool = db::init_pool(&database_url);

       // Build our application with a route
       let app = build_app(pool);

       // Run the Axum server
       let addr = "0.0.0.0:3000".parse().unwrap();
       println!("Listening on {}", addr);
       axum::Server::bind(&addr)
           .serve(app.into_make_service())
           .await
           .unwrap();
   }
   ```
3. Commit:
   ```bash
   git add src/main.rs
   git commit -m "Add Axum routes for create/list/get items"
   ```

---

## 6. Simple Review Endpoint & Scheduling Logic

### Step H: Add a `reviews` table and model
1. **Generate the migration**:
   ```bash
   diesel migration generate create_reviews
   ```
2. **Edit** `up.sql`:
   ```sql
   CREATE TABLE reviews (
       id TEXT PRIMARY KEY,
       item_id TEXT NOT NULL,
       rating INTEGER NOT NULL,
       review_timestamp DATETIME NOT NULL
   );
   ```
   **Edit** `down.sql`:
   ```sql
   DROP TABLE reviews;
   ```
3. **Run migration**:
   ```bash
   diesel migration run
   ```
4. **Regenerate or manually edit `src/schema.rs`** to include `reviews`.
5. **Create `Review` model** in `src/models.rs`:
   ```rust
   #[derive(Queryable, Insertable, Debug)]
   #[diesel(table_name = reviews)]
   pub struct Review {
       pub id: String,
       pub item_id: String,
       pub rating: i32,
       pub review_timestamp: DateTime<Utc>,
   }

   impl Review {
       pub fn new(item_id: &str, rating: i32) -> Self {
           Self {
               id: Uuid::new_v4().to_string(),
               item_id: item_id.to_string(),
               rating,
               review_timestamp: chrono::Utc::now(),
           }
       }
   }
   ```
6. **Commit**:
   ```bash
   git add .
   git commit -m "Add reviews table and model"
   ```

### Step I: Implement Simple Scheduling in `repo`
1. **Extend `repo.rs`** with a function to record a review:
   ```rust
   pub fn record_review(pool: &DbPool, item_id_val: &str, rating_val: i32) -> Result<Review> {
       let conn = &mut pool.get()?;
       // 1) Insert the review record
       let new_review = Review::new(item_id_val, rating_val);
       diesel::insert_into(reviews::table)
           .values(&new_review)
           .execute(conn)?;

       // 2) Retrieve the item and update next_review
       let mut item = items
           .filter(id.eq(item_id_val))
           .first::<Item>(conn)?;

       let now = chrono::Utc::now();
       item.last_review = Some(now);
       // Simple scheduling example:
       // rating 1 => add 1 day, rating 2 => 3 days, rating 3 => 7 days
       let add_days = match rating_val {
           1 => 1,
           2 => 3,
           3 => 7,
           _ => 1,
       };
       item.next_review = Some(now + chrono::Duration::days(add_days));

       diesel::update(items.filter(id.eq(item_id_val)))
           .set((
               next_review.eq(item.next_review),
               last_review.eq(item.last_review),
               updated_at.eq(now),
           ))
           .execute(conn)?;

       Ok(new_review)
   }
   ```
2. **Commit**:
   ```bash
   git add src/repo.rs
   git commit -m "Add simple record_review function with naive scheduling"
   ```

### Step J: Add a `POST /reviews` Endpoint
1. **In your routes**, add:
   ```rust
   #[derive(Deserialize)]
   struct CreateReviewDto {
       item_id: String,
       rating: i32,
   }

   async fn create_review_handler(
       State(pool): State<DbPool>,
       Json(payload): Json<CreateReviewDto>,
   ) -> Json<Review> {
       let review = repo::record_review(&pool, &payload.item_id, payload.rating)
           .expect("Failed to record review");
       Json(review)
   }
   ```
2. **Add the route** in `build_app`:
   ```rust
   Router::new()
       // ...
       .route("/reviews", post(create_review_handler))
       // ...
   ```
3. **Commit**:
   ```bash
   git add src/main.rs
   git commit -m "Add /reviews endpoint"
   ```

---

## 7. Testing

### Step K: Basic Unit Tests
1. **Create `tests` directory** or use `mod tests` in code. Example in `tests/integration.rs`:
   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use crate::db::init_pool;
       use crate::repo;
       use std::env;

       #[test]
       fn test_create_item() {
           let pool = init_pool(":memory:");
           // run migrations in-memory if you prefer
           let item = repo::create_item(&pool, "Test item".to_string()).unwrap();
           assert_eq!(item.title, "Test item");
       }
   }
   ```
2. **Commit**:
   ```bash
   git add tests/
   git commit -m "Add basic integration test for create_item"
   ```

### Step L: Integration Test for Review Endpoint
1. **Use `tokio::test`** with an in-memory DB and a test server:
   ```rust
   #[tokio::test]
   async fn test_create_review() {
       // Spin up Axum with in-memory DB, call /reviews
       // Check if the item’s next_review was updated
   }
   ```
2. **Commit**:
   ```bash
   git add tests/integration.rs
   git commit -m "Add integration test for review flow"
   ```

---

## 8. Review & Next Steps

At this point, you have:
- A working Rust project with Diesel + SQLite
- An `items` table and a `reviews` table
- Basic endpoints for creating items, listing them, and recording reviews
- A naive scheduling approach (the MVP for verifying SRS logic)
- A test suite you can begin expanding

From here, you can:
- Add more thorough tests
- Expand the scheduling algorithm
- Introduce a simple authentication mechanism if desired
- Gradually add more features from the design spec (e.g. more item fields, type registry, tags, etc.)

---

# Summary

**Overall MVP Steps**:

1. **Project Initialization**: `cargo new`, set up Git
2. **Dependencies**: Add Axum, Diesel, Serde, etc.
3. **Database Migrations**: `items` table, `reviews` table
4. **Models**: `Item` and `Review` struct
5. **Repository**: Basic CRUD (create/read items, record review)
6. **API**: Axum endpoints (`POST /items`, `GET /items`, `GET /items/:id`, `POST /reviews`)
7. **Simple Scheduling**: Hardcode logic to set `next_review`
8. **Tests**: Write both unit and integration tests

Following this plan allows your LLM agent to produce a series of well-defined commits and a minimal but functional spaced repetition server.