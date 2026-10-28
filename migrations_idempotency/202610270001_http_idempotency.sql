-- HTTP idempotency ledger. FACTSTR remains authoritative for domain facts.
-- request_fingerprint is a SHA-256 hex digest; raw request payloads are never stored.
-- In-progress/unknown entries are intentionally not auto-expired: deleting one
-- could re-execute a command whose FACTSTR append succeeded before a process crash.
CREATE TABLE IF NOT EXISTS http_idempotency_keys (
    operation_scope TEXT NOT NULL,
    idempotency_key TEXT NOT NULL CHECK (octet_length(idempotency_key) BETWEEN 1 AND 200),
    request_fingerprint TEXT NOT NULL CHECK (char_length(request_fingerprint) = 64),
    state TEXT NOT NULL CHECK (state IN ('in_progress', 'completed', 'unknown')),
    owner_token UUID NOT NULL,
    response_status SMALLINT,
    response_headers JSONB,
    response_body BYTEA,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (now() + INTERVAL '24 hours'),
    PRIMARY KEY (operation_scope, idempotency_key),
    CHECK (
        (state = 'completed'
            AND response_status IS NOT NULL
            AND response_status BETWEEN 100 AND 599
            AND response_headers IS NOT NULL
            AND response_body IS NOT NULL)
        OR
        (state IN ('in_progress', 'unknown')
            AND response_status IS NULL
            AND response_headers IS NULL
            AND response_body IS NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_http_idempotency_completed_expiry
    ON http_idempotency_keys (expires_at)
    WHERE state = 'completed';

CREATE INDEX IF NOT EXISTS idx_http_idempotency_pending_age
    ON http_idempotency_keys (updated_at)
    WHERE state IN ('in_progress', 'unknown');
