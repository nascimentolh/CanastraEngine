-- What a character carries. Items go with the character when it is deleted.
CREATE TABLE character_items (
    id BIGSERIAL PRIMARY KEY,
    character_id BIGINT NOT NULL REFERENCES characters (id) ON DELETE CASCADE,
    item_id INTEGER NOT NULL,
    count BIGINT NOT NULL,
    equipped BOOLEAN NOT NULL,
    -- When the item disappears; NULL keeps it forever.
    expires_at TIMESTAMPTZ
);

CREATE INDEX character_items_character ON character_items (character_id);
