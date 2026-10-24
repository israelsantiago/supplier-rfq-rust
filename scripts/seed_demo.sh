#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:3000}"
RFQ_ID="11111111-1111-4111-8111-111111111111"
SUPPLIER_ID="22222222-2222-4222-8222-222222222222"
ITEM_A_ID="44444444-4444-4444-8444-444444444441"
ITEM_B_ID="44444444-4444-4444-8444-444444444442"

curl --fail-with-body -sS -X POST "$BASE_URL/api/rfqs" \
  -H 'Content-Type: application/json' \
  --data-binary @- <<JSON
{
  "id": "$RFQ_ID",
  "reference": "RFQ-DEMO-2026-001",
  "currency": "BRL",
  "items": [
    {"id": "$ITEM_A_ID", "sku": "SKU-A", "description": "Componente industrial A", "required_quantity": 10},
    {"id": "$ITEM_B_ID", "sku": "SKU-B", "description": "Componente industrial B", "required_quantity": 5}
  ]
}
JSON

echo
curl --fail-with-body -sS -X POST "$BASE_URL/api/suppliers" \
  -H 'Content-Type: application/json' \
  --data-binary @- <<JSON
{
  "id": "$SUPPLIER_ID",
  "reference": "SUP-DEMO",
  "name": "Fornecedor Demonstracao"
}
JSON
echo
curl --fail-with-body -sS -X PUT "$BASE_URL/api/suppliers/$SUPPLIER_ID/eligibility" \
  -H 'Content-Type: application/json' \
  -d '{"status":"eligible"}'
echo
printf 'Demo facts appended to FACTSTR. RFQ_ID=%s SUPPLIER_ID=%s\n' "$RFQ_ID" "$SUPPLIER_ID"
