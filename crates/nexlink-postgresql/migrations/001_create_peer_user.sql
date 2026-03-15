CREATE TABLE IF NOT EXISTS peer_user (
    id          BIGSERIAL    PRIMARY KEY,
    peer_id     VARCHAR(128) NOT NULL,
    send        BIGINT       NOT NULL DEFAULT 0,
    recv        BIGINT       NOT NULL DEFAULT 0,
    total_limit BIGINT       NOT NULL DEFAULT 0,
    is_valid    BOOLEAN      NOT NULL DEFAULT FALSE,
    updated_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_peer_user_peer_id ON peer_user (peer_id);
