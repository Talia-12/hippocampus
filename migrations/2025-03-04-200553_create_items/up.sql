CREATE TABLE items (
    id TEXT NOT NULL PRIMARY KEY,
    title TEXT NOT NULL,
    next_review TIMESTAMP,
    last_review TIMESTAMP,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
