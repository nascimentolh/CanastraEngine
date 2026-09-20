-- Which way a character faces, in Unreal rotation units: 65536 to a full turn, 0 along +X.
ALTER TABLE characters ADD COLUMN heading INTEGER NOT NULL DEFAULT 0;
