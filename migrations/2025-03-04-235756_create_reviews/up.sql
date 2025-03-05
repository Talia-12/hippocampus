-- Your SQL goes here

CREATE TABLE reviews (
    id TEXT PRIMARY KEY,
    item_id TEXT NOT NULL,
    rating INTEGER NOT NULL,
    review_timestamp DATETIME NOT NULL
);
