CREATE TABLE reviews (
    id TEXT NOT NULL PRIMARY KEY,
    card_id TEXT NOT NULL,
    rating INTEGER NOT NULL,
    -- Store as Unix timestamp (seconds since epoch)
    review_timestamp TIMESTAMP NOT NULL,
    
    FOREIGN KEY (card_id) REFERENCES cards(id)
);

CREATE INDEX reviews_card_id_index ON reviews(card_id);