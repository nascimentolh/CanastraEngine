CREATE TABLE characters (
    id BIGSERIAL PRIMARY KEY,
    account_id BIGINT NOT NULL REFERENCES accounts (id),
    -- The game server the character lives on.
    server_id SMALLINT NOT NULL,
    name TEXT NOT NULL,
    class_id SMALLINT NOT NULL,
    female BOOLEAN NOT NULL,
    hair_style SMALLINT NOT NULL,
    hair_color SMALLINT NOT NULL,
    face SMALLINT NOT NULL,
    level SMALLINT NOT NULL DEFAULT 1,
    x INTEGER NOT NULL,
    y INTEGER NOT NULL,
    z INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Names are unique on a server regardless of case.
CREATE UNIQUE INDEX characters_name ON characters (server_id, lower(name));
CREATE INDEX characters_account ON characters (account_id, server_id);
