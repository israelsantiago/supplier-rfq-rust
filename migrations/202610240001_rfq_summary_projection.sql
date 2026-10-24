-- Rebuildable, application-owned read model. FACTSTR remains the source of truth.
CREATE TABLE IF NOT EXISTS rfq_summary_projection (
    rfq_id UUID PRIMARY KEY,
    reference TEXT NOT NULL UNIQUE,
    currency TEXT NOT NULL CHECK (char_length(currency) = 3),
    status TEXT NOT NULL CHECK (status IN ('open', 'closed')),
    opened_at TIMESTAMPTZ NOT NULL,
    closed_at TIMESTAMPTZ,
    item_count INTEGER NOT NULL CHECK (item_count > 0),
    quote_count BIGINT NOT NULL DEFAULT 0 CHECK (quote_count >= 0),
    lowest_quote_amount_cents BIGINT,
    lowest_quote_supplier_id UUID,
    last_event_sequence BIGINT NOT NULL CHECK (last_event_sequence > 0),
    projected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (
        (status = 'open' AND closed_at IS NULL)
        OR (status = 'closed' AND closed_at IS NOT NULL)
    ),
    CHECK (
        (quote_count = 0 AND lowest_quote_amount_cents IS NULL AND lowest_quote_supplier_id IS NULL)
        OR (quote_count > 0 AND lowest_quote_amount_cents IS NOT NULL AND lowest_quote_supplier_id IS NOT NULL)
    ),
    CHECK (lowest_quote_amount_cents IS NULL OR lowest_quote_amount_cents >= 0)
);

CREATE INDEX IF NOT EXISTS idx_rfq_summary_status_opened
    ON rfq_summary_projection (status, opened_at DESC, rfq_id);

CREATE INDEX IF NOT EXISTS idx_rfq_summary_opened
    ON rfq_summary_projection (opened_at DESC, rfq_id);
