CREATE TABLE accounts (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    -- Argon2id in PHC string form.
    password_hash TEXT NOT NULL,
    banned BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_login_at TIMESTAMPTZ
);

-- Names are unique regardless of case.
CREATE UNIQUE INDEX accounts_name ON accounts (lower(name));
