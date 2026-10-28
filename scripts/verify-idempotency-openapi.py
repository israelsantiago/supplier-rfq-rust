#!/usr/bin/env python3
"""Guard that Idempotency-Key appears only on the five supported command operations."""
from __future__ import annotations
import re
from pathlib import Path

path = Path("docs/openapi.yaml")
if not path.is_file():
    raise SystemExit("ERRO: docs/openapi.yaml não encontrado.")
source = path.read_text(encoding="utf-8")
expected = {
    "openRfq": ("/api/rfqs", "post"),
    "registerSupplier": ("/api/suppliers", "post"),
    "setSupplierEligibility": ("/api/suppliers/{supplier_id}/eligibility", "put"),
    "submitSupplierQuote": ("/api/rfqs/{rfq_id}/quotes", "post"),
    "closeRfq": ("/api/rfqs/{rfq_id}/close", "post"),
}
operations: dict[str, tuple[str, str, str]] = {}
route: str | None = None
method: str | None = None
operation_id: str | None = None
buffer: list[str] = []

def save() -> None:
    if operation_id is not None and route is not None and method is not None:
        operations[operation_id] = (route, method, "\n".join(buffer))

for line in source.splitlines():
    if re.match(r"^  /[^:]+:$", line):
        save(); route = line.strip()[:-1]; method = None; operation_id = None; buffer = []
    elif re.match(r"^    (get|post|put|patch|delete):$", line):
        save(); method = line.strip()[:-1]; operation_id = None; buffer = []
    elif line.startswith("      operationId:"):
        save(); operation_id = line.split(":", 1)[1].strip(); buffer = [line]
    elif operation_id is not None:
        buffer.append(line)
save()

actual = {
    op_id: (op_route, op_method)
    for op_id, (op_route, op_method, block) in operations.items()
    if "#/components/parameters/IdempotencyKey" in block
}
if actual != expected:
    raise SystemExit(
        "ERRO: escopo OpenAPI de Idempotency-Key divergente. "
        f"Esperado={dict(sorted(expected.items()))}; encontrado={dict(sorted(actual.items()))}"
    )
if "Idempotency-Key" not in source or "in: header" not in source:
    raise SystemExit("ERRO: definição do parâmetro de header Idempotency-Key ausente.")
print("OK: Idempotency-Key aparece exatamente nos cinco comandos, nunca em GET/query.")
