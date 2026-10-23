CREATE TABLE rfqs (
    id UUID PRIMARY KEY,
    reference TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL CHECK (status IN ('draft', 'open', 'closed')),
    currency VARCHAR(3) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    opened_at TIMESTAMPTZ,
    closed_at TIMESTAMPTZ,
    CHECK (
        (status = 'draft' AND opened_at IS NULL AND closed_at IS NULL)
        OR (status = 'open' AND opened_at IS NOT NULL AND closed_at IS NULL)
        OR (status = 'closed' AND opened_at IS NOT NULL AND closed_at IS NOT NULL)
    )
);

CREATE TABLE suppliers (
    id UUID PRIMARY KEY,
    reference TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('eligible', 'blocked'))
);

CREATE TABLE rfq_items (
    id UUID PRIMARY KEY,
    rfq_id UUID NOT NULL REFERENCES rfqs(id),
    sku TEXT NOT NULL,
    description TEXT NOT NULL,
    required_quantity BIGINT NOT NULL CHECK (required_quantity > 0),
    UNIQUE (rfq_id, sku),
    UNIQUE (rfq_id, id)
);

CREATE TABLE supplier_quotes (
    id UUID PRIMARY KEY,
    rfq_id UUID NOT NULL REFERENCES rfqs(id),
    supplier_id UUID NOT NULL REFERENCES suppliers(id),
    currency VARCHAR(3) NOT NULL,
    total_amount_cents BIGINT NOT NULL CHECK (total_amount_cents > 0),
    delivery_days INTEGER NOT NULL CHECK (delivery_days > 0),
    valid_until DATE NOT NULL,
    submitted_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT supplier_quotes_rfq_supplier_key UNIQUE (rfq_id, supplier_id),
    CONSTRAINT supplier_quotes_rfq_id_key UNIQUE (rfq_id, id)
);

CREATE TABLE supplier_quote_items (
    quote_id UUID NOT NULL,
    rfq_id UUID NOT NULL,
    rfq_item_id UUID NOT NULL,
    quantity BIGINT NOT NULL CHECK (quantity > 0),
    unit_price_cents BIGINT NOT NULL CHECK (unit_price_cents > 0),
    line_total_cents BIGINT NOT NULL CHECK (line_total_cents > 0),
    PRIMARY KEY (quote_id, rfq_item_id),
    CONSTRAINT quote_item_quote_same_rfq_fk
        FOREIGN KEY (rfq_id, quote_id) REFERENCES supplier_quotes(rfq_id, id) ON DELETE CASCADE,
    CONSTRAINT quote_item_rfq_item_same_rfq_fk
        FOREIGN KEY (rfq_id, rfq_item_id) REFERENCES rfq_items(rfq_id, id)
);

CREATE INDEX supplier_quotes_rfq_submitted_idx
    ON supplier_quotes (rfq_id, submitted_at, id);


-- RFQ item membership is frozen once the RFQ is opened. The trigger takes an
-- UPDATE lock on the parent RFQ row so item changes cannot race with a command
-- that has read/locked the RFQ context using FOR SHARE.
CREATE FUNCTION enforce_rfq_items_mutable_only_in_draft()
RETURNS TRIGGER AS $$
DECLARE
    target_rfq_id UUID;
    current_status TEXT;
BEGIN
    IF TG_OP = 'DELETE' THEN
        target_rfq_id := OLD.rfq_id;
    ELSE
        target_rfq_id := NEW.rfq_id;
    END IF;

    IF TG_OP = 'UPDATE' AND NEW.rfq_id <> OLD.rfq_id THEN
        RAISE EXCEPTION 'An RFQ item cannot be moved between RFQs'
            USING ERRCODE = '23514', CONSTRAINT = 'rfq_item_rfq_is_immutable';
    END IF;

    SELECT status INTO current_status
    FROM rfqs
    WHERE id = target_rfq_id
    FOR UPDATE;

    IF current_status IS DISTINCT FROM 'draft' THEN
        RAISE EXCEPTION 'RFQ items can only change while the RFQ is draft'
            USING ERRCODE = '23514', CONSTRAINT = 'rfq_items_mutable_only_in_draft';
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER rfq_items_mutable_only_in_draft
BEFORE INSERT OR UPDATE OR DELETE ON rfq_items
FOR EACH ROW EXECUTE FUNCTION enforce_rfq_items_mutable_only_in_draft();
