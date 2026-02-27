CREATE TABLE item_relations (
    parent_item_id TEXT NOT NULL,
    child_item_id TEXT NOT NULL,
    relation_type TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (parent_item_id, child_item_id),
    FOREIGN KEY (parent_item_id) REFERENCES items(id) ON DELETE CASCADE,
    FOREIGN KEY (child_item_id) REFERENCES items(id) ON DELETE CASCADE
);
CREATE INDEX item_relations_child_item_id_index ON item_relations(child_item_id);
