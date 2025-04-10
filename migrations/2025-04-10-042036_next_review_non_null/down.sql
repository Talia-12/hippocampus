-- This file should undo anything in `up.sql`
-- SQLite doesn't support ALTER COLUMN directly, so we need to use the same approach as in up.sql

-- Create a new table with the original structure (next_review nullable)
CREATE TABLE cards_new (
    id TEXT NOT NULL PRIMARY KEY,
    item_id TEXT NOT NULL,
    card_index INTEGER NOT NULL,
    next_review TIMESTAMP,
    last_review TIMESTAMP,
    scheduler_data TEXT,
    priority REAL NOT NULL DEFAULT 0.5 CHECK (priority >= 0 AND priority <= 1),
    suspended TIMESTAMP DEFAULT NULL,
    
    FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE CASCADE,
    UNIQUE(item_id, card_index)
);

-- Copy data from the current table to the new one
INSERT INTO cards_new SELECT * FROM cards;

-- Drop the current table
DROP TABLE cards;

-- Rename the new table to the original name
ALTER TABLE cards_new RENAME TO cards;

-- Recreate the index
CREATE INDEX cards_item_id_index ON cards(item_id);
