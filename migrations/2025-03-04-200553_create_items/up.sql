CREATE TABLE items (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    next_review DATETIME,
    last_review DATETIME,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NOT NULL
);
