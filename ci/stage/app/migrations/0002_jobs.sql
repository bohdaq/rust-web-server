-- The job queue (see src/db.rs) and the rows its jobs write.
CREATE TABLE IF NOT EXISTS e2e_jobs (
    id          BIGSERIAL PRIMARY KEY,
    job_key     TEXT NOT NULL,
    fail_first  BOOLEAN NOT NULL DEFAULT FALSE,
    status      TEXT NOT NULL DEFAULT 'pending',
    attempts    BIGINT NOT NULL DEFAULT 0,
    max_retries BIGINT NOT NULL DEFAULT 3,
    next_run_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    locked_by   TEXT,
    locked_at   TIMESTAMPTZ,
    last_error  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS e2e_jobs_due ON e2e_jobs (status, next_run_at);
CREATE TABLE IF NOT EXISTS job_runs (
    id       BIGSERIAL PRIMARY KEY,
    job_key  TEXT NOT NULL,
    pod      TEXT NOT NULL,
    attempts BIGINT NOT NULL,
    at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS job_runs_key ON job_runs (job_key);
