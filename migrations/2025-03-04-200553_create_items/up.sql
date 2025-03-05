CREATE TABLE item_types (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at TIMESTAMP NOT NULL
);

CREATE TABLE items (
    id TEXT NOT NULL PRIMARY KEY,
    item_type TEXT NOT NULL,
    title TEXT NOT NULL UNIQUE,
    -- This data will be stored as JSON
    item_data TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,

    FOREIGN KEY (item_type) REFERENCES item_types(id)
);

CREATE TABLE cards (
    id TEXT NOT NULL PRIMARY KEY,
    item_id TEXT NOT NULL,
    card_index INTEGER NOT NULL,
    next_review TIMESTAMP,
    last_review TIMESTAMP,
    -- This data will be stored as JSON
    scheduler_data TEXT,
    
    FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE CASCADE,
    UNIQUE(item_id, card_index)
);

CREATE INDEX cards_item_id_index ON cards(item_id);

CREATE TABLE tags (
    id TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at TIMESTAMP NOT NULL
);

CREATE TABLE item_tags (
    item_id TEXT NOT NULL,
    tag_id TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    
    PRIMARY KEY (item_id, tag_id),
    FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE INDEX item_tags_tag_id_index ON item_tags(tag_id);
CREATE INDEX item_tags_item_id_index ON item_tags(item_id);