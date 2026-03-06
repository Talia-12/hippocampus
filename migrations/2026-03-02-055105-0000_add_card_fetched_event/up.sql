-- Add cached event chain result and timestamps to cards
ALTER TABLE cards ADD COLUMN card_data TEXT;
ALTER TABLE cards ADD COLUMN updated_at TIMESTAMP NOT NULL DEFAULT '1970-01-01 00:00:00';
ALTER TABLE cards ADD COLUMN cache_updated_at TIMESTAMP;

-- Set existing cards' updated_at to current time
UPDATE cards SET updated_at = strftime('%Y-%m-%d %H:%M:%f', 'now');

-- Add updated_at to item_types
ALTER TABLE item_types ADD COLUMN updated_at TIMESTAMP NOT NULL DEFAULT '1970-01-01 00:00:00';

-- Set existing item_types' updated_at to current time
UPDATE item_types SET updated_at = strftime('%Y-%m-%d %H:%M:%f', 'now');

-- Trigger: auto-update item_types.updated_at when name or review_function changes
CREATE TRIGGER update_item_type_updated_at
AFTER UPDATE ON item_types
WHEN (OLD.name IS NOT NEW.name OR OLD.review_function IS NOT NEW.review_function)
BEGIN
    UPDATE item_types SET updated_at = strftime('%Y-%m-%d %H:%M:%f', 'now') WHERE id = NEW.id;
END;

-- Create card_fetched_events table
-- order_index is checked non-negative in SQL so it mirrors the Rust-side
-- OrderIndex(u16) newtype: any bypass of the type guard still hits the CHECK.
CREATE TABLE card_fetched_events (
    item_type_id TEXT NOT NULL REFERENCES item_types(id),
    order_index INTEGER NOT NULL CHECK (order_index >= 0),
    function_name TEXT NOT NULL,
    PRIMARY KEY (item_type_id, order_index),
    UNIQUE (item_type_id, function_name)
);

-- Triggers: bump item_types.updated_at when events are added/removed
CREATE TRIGGER update_item_type_on_event_insert
AFTER INSERT ON card_fetched_events
BEGIN
    UPDATE item_types SET updated_at = strftime('%Y-%m-%d %H:%M:%f', 'now') WHERE id = NEW.item_type_id;
END;

CREATE TRIGGER update_item_type_on_event_update
AFTER UPDATE ON card_fetched_events
BEGIN
    UPDATE item_types SET updated_at = strftime('%Y-%m-%d %H:%M:%f', 'now') WHERE id = NEW.item_type_id;
END;

CREATE TRIGGER update_item_type_on_event_delete
AFTER DELETE ON card_fetched_events
BEGIN
    UPDATE item_types SET updated_at = strftime('%Y-%m-%d %H:%M:%f', 'now') WHERE id = OLD.item_type_id;
END;

-- Trigger: auto-update cards.updated_at when core fields change.
--
-- Excluded fields:
--   * card_data / cache_updated_at — these ARE the cache, and bumping
--     updated_at on a cache write would immediately re-mark the cache stale.
--   * priority_offset — rewritten daily en masse by
--     `regenerate_priority_offsets`. Including it here would force every
--     card's event chain to recompute on the first fetch of each day even
--     when no chain function reads priority_offset, turning a cheap daily
--     shuffle into an O(N) chain-recompute storm.
CREATE TRIGGER update_card_updated_at
AFTER UPDATE ON cards
WHEN (OLD.id IS NOT NEW.id OR OLD.item_id IS NOT NEW.item_id OR
      OLD.card_index IS NOT NEW.card_index OR OLD.next_review IS NOT NEW.next_review OR
      OLD.last_review IS NOT NEW.last_review OR OLD.scheduler_data IS NOT NEW.scheduler_data OR
      OLD.priority IS NOT NEW.priority OR OLD.suspended IS NOT NEW.suspended OR
      OLD.sort_position IS NOT NEW.sort_position)
BEGIN
    UPDATE cards SET updated_at = strftime('%Y-%m-%d %H:%M:%f', 'now') WHERE id = NEW.id;
END;

-- Trigger: auto-update items.updated_at when non-timestamp fields change
CREATE TRIGGER update_item_updated_at
AFTER UPDATE ON items
WHEN (OLD.item_type IS NOT NEW.item_type OR OLD.title IS NOT NEW.title OR
      OLD.item_data IS NOT NEW.item_data)
BEGIN
    UPDATE items SET updated_at = strftime('%Y-%m-%d %H:%M:%f', 'now') WHERE id = NEW.id;
END;
