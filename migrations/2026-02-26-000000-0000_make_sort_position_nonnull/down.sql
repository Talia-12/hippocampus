-- Recreate table with sort_position nullable
CREATE TABLE cards_new (
    id TEXT NOT NULL PRIMARY KEY,
    item_id TEXT NOT NULL,
    card_index INTEGER NOT NULL,
    next_review TIMESTAMP NOT NULL,
    last_review TIMESTAMP,
    scheduler_data TEXT,
    priority REAL NOT NULL DEFAULT 0.5 CHECK (priority >= 0 AND priority <= 1),
    suspended TIMESTAMP DEFAULT NULL,
    sort_position REAL,
    priority_offset REAL NOT NULL DEFAULT 0.0,
    FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE CASCADE,
    UNIQUE(item_id, card_index)
);

INSERT INTO cards_new SELECT * FROM cards;
DROP TABLE cards;
ALTER TABLE cards_new RENAME TO cards;
CREATE INDEX cards_item_id_index ON cards(item_id);

-- Restore original values: negate non-zero positions, set 0.0 back to NULL
UPDATE cards SET sort_position = CASE
    WHEN sort_position = 0.0 THEN NULL
    ELSE -sort_position
END;
