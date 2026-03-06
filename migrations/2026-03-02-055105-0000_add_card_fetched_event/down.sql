-- Drop triggers
DROP TRIGGER IF EXISTS update_item_updated_at;
DROP TRIGGER IF EXISTS update_card_updated_at;
DROP TRIGGER IF EXISTS update_item_type_on_event_delete;
DROP TRIGGER IF EXISTS update_item_type_on_event_insert;
DROP TRIGGER IF EXISTS update_item_type_on_event_update;
DROP TRIGGER IF EXISTS update_item_type_updated_at;

-- Drop card_fetched_events table
DROP TABLE IF EXISTS card_fetched_events;

-- SQLite doesn't support DROP COLUMN before 3.35.0, so we recreate tables

-- Recreate item_types without updated_at
CREATE TABLE item_types_backup AS SELECT id, name, created_at, review_function FROM item_types;
DROP TABLE item_types;
CREATE TABLE item_types (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    review_function TEXT NOT NULL
);
INSERT INTO item_types SELECT * FROM item_types_backup;
DROP TABLE item_types_backup;

-- Recreate cards without card_data, updated_at, cache_updated_at
CREATE TABLE cards_backup AS SELECT id, item_id, card_index, next_review, last_review, scheduler_data, priority, suspended, sort_position, priority_offset FROM cards;
DROP TABLE cards;
CREATE TABLE cards (
    id TEXT PRIMARY KEY NOT NULL,
    item_id TEXT NOT NULL REFERENCES items(id),
    card_index INTEGER NOT NULL,
    next_review TIMESTAMP NOT NULL,
    last_review TIMESTAMP,
    scheduler_data TEXT,
    priority FLOAT NOT NULL,
    suspended TIMESTAMP,
    sort_position FLOAT,
    priority_offset FLOAT NOT NULL DEFAULT 0.0
);
INSERT INTO cards SELECT * FROM cards_backup;
DROP TABLE cards_backup;
