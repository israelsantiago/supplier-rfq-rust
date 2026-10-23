-- Deterministic demo fixture. Run after `cargo run` has applied migrations.
-- This script intentionally resets the RFQ fixture and its quotes when rerun.
INSERT INTO rfqs (id, reference, status, currency)
VALUES ('11111111-1111-4111-8111-111111111111', 'RFQ-DEMO-2026-001', 'draft', 'BRL')
ON CONFLICT (id) DO NOTHING;

UPDATE rfqs
SET status = 'draft', opened_at = NULL, closed_at = NULL
WHERE id = '11111111-1111-4111-8111-111111111111';

DELETE FROM supplier_quotes WHERE rfq_id = '11111111-1111-4111-8111-111111111111';
DELETE FROM rfq_items WHERE rfq_id = '11111111-1111-4111-8111-111111111111';

INSERT INTO suppliers (id, reference, name, status)
VALUES
 ('22222222-2222-4222-8222-222222222222', 'SUP-BETA', 'Fornecedor Beta Ltda.', 'eligible'),
 ('33333333-3333-4333-8333-333333333333', 'SUP-BLOCKED', 'Fornecedor Não Homologado Ltda.', 'blocked')
ON CONFLICT (id) DO UPDATE SET status = EXCLUDED.status, name = EXCLUDED.name;

INSERT INTO rfq_items (id, rfq_id, sku, description, required_quantity)
VALUES
 ('44444444-4444-4444-8444-444444444441', '11111111-1111-4111-8111-111111111111', 'CABO-10M', 'Cabo industrial de 10 metros', 10),
 ('44444444-4444-4444-8444-444444444442', '11111111-1111-4111-8111-111111111111', 'CONECTOR-IP67', 'Conector industrial IP67', 5);

UPDATE rfqs
SET status = 'open', opened_at = now(), closed_at = NULL
WHERE id = '11111111-1111-4111-8111-111111111111';
