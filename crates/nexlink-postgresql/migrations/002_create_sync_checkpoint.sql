CREATE TABLE IF NOT EXISTS sync_checkpoint (
    service_name VARCHAR(128) PRIMARY KEY,
    last_ts      TIMESTAMPTZ,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
