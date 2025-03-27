-- Your SQL goes here
ALTER TABLE cards ADD COLUMN priority REAL NOT NULL DEFAULT 0.5 CHECK (priority >= 0 AND priority <= 1);
