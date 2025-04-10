-- Your SQL goes here
-- First, update any existing null values with a default timestamp
UPDATE cards
SET next_review = CURRENT_TIMESTAMP
WHERE next_review IS NULL;

-- SQLite doesn't support ALTER COLUMN directly, so we need to:
-- 1. Create a new table with the desired schema
-- 2. Copy data from the old table to the new one
-- 3. Drop the old table
-- 4. Rename the new table to the original name

-- Create a new table with the same structure but with next_review NOT NULL constraint
CREATE TABLE cards_new (
    id TEXT NOT NULL PRIMARY KEY,
    item_id TEXT NOT NULL,
    card_index INTEGER NOT NULL,
    next_review TIMESTAMP NOT NULL,
    last_review TIMESTAMP,
    scheduler_data TEXT,
    priority REAL NOT NULL DEFAULT 0.5 CHECK (priority >= 0 AND priority <= 1),
    suspended TIMESTAMP DEFAULT NULL,
    
    FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE CASCADE,
    UNIQUE(item_id, card_index)
);

-- Copy data from the old table to the new one
INSERT INTO cards_new SELECT * FROM cards;

-- Drop the old table
DROP TABLE cards;

-- Rename the new table to the original name
ALTER TABLE cards_new RENAME TO cards;

-- Recreate the index
CREATE INDEX cards_item_id_index ON cards(item_id);
