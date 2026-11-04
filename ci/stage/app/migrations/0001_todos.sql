-- BIGINT/BIGSERIAL only: rws's Postgres row decoder reads INT4 columns as NULL.
CREATE TABLE IF NOT EXISTS todos (
    id    BIGSERIAL PRIMARY KEY,
    title TEXT NOT NULL
);
