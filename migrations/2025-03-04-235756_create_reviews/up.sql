CREATE TABLE reviews (
    id TEXT NOT NULL PRIMARY KEY,
    item_id TEXT NOT NULL,
    rating INTEGER NOT NULL,
    -- Store as Unix timestamp (seconds since epoch)
    review_timestamp TIMESTAMP NOT NULL
);
