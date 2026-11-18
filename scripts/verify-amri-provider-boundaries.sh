#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

python3 - <<'PY'
from pathlib import Path
import re

required = (
    Path("src/app/amri/providers/mod.rs"),
    Path("src/app/amri/providers/testing.rs"),
    Path("tests/external_provider_contracts.rs"),
    Path("tests/wiremock-rs/src/main.rs"),
    Path("tests/contracts/external-providers-v1.openapi.yaml"),
)
for path in required:
    if not path.is_file() or path.stat().st_size == 0:
        raise SystemExit(f"FAIL: arquivo ausente/vazio: {path}")

core_files = [*Path("src/amri").rglob("*.rs"), *Path("src/domain").rglob("*.rs")]
participant_files = list(Path("src/amri_participants").rglob("*.rs"))
forbidden_core = re.compile(r"wiremock|testcontainers|reqwest|crate::app::amri::providers|crate::app::amri::provider_test_support", re.I)
for path in core_files:
    source = path.read_text(encoding="utf-8")
    if forbidden_core.search(source):
        raise SystemExit(f"FAIL: dependência externa/Application encontrada no Core/Functional Core: {path}")
for path in participant_files:
    source = path.read_text(encoding="utf-8")
    if forbidden_core.search(source):
        raise SystemExit(f"FAIL: Prepared Participant depende de Provider/HTTP: {path}")

root_manifest = __import__("tomllib").loads(Path("Cargo.toml").read_text(encoding="utf-8"))
if "testcontainers" in root_manifest.get("dependencies", {}):
    raise SystemExit("FAIL: Testcontainers não pode ser dependência de produção")
if root_manifest.get("dev-dependencies", {}).get("testcontainers") != "=0.27.3":
    raise SystemExit("FAIL: Testcontainers Rust deve ser dev-dependency direta e pinada em 0.27.3")
providers = Path("src/app/amri/providers/mod.rs").read_text(encoding="utf-8")
for token in ("pub trait ErpReferenceProvider", "pub trait ComplianceProvider", "pub trait TaxProvider", "NotFound", "NotApplicable", "Unavailable", "TimedOut", "InvalidResponse", "EvidenceFreshness", "Stale", "freshness_at", "observed_at", "expires_at"):
    if token not in providers:
        raise SystemExit(f"FAIL: contrato de Provider incompleto: {token}")
for forbidden in ("factstr::", "sqlx::", "EventStore", "append_if", "reqwest::", "wiremock::", "testcontainers"):
    if forbidden in providers:
        raise SystemExit(f"FAIL: porta Provider contém dependência/efeito proibido: {forbidden}")

amri = Path("src/app/amri/mod.rs").read_text(encoding="utf-8")
if "pub(crate) mod providers;" not in amri:
    raise SystemExit("FAIL: módulo Providers deve permanecer pub(crate)")
if '#[cfg(feature = "test-support")]' not in amri or "provider_test_support" not in amri:
    raise SystemExit("FAIL: reexport de teste deve permanecer feature-gated")

for path in Path("src").rglob("*.rs"):
    source = path.read_text(encoding="utf-8")
    if path.parts[:2] == ("src", "amri") or path.parts[:2] == ("src", "domain"):
        continue
    if path.parts[:3] == ("src", "app", "amri") and "providers" in path.parts:
        continue
    if re.search(r"wiremock::|testcontainers_modules|GenericImage::new", source):
        raise SystemExit(f"FAIL: infraestrutura WireMock/Testcontainers vazou para produção: {path}")

openapi = Path("tests/contracts/external-providers-v1.openapi.yaml").read_text(encoding="utf-8")
for path in ("/v1/erp-reference/suppliers/{supplier_id}/reference", "/v1/compliance/suppliers/{supplier_id}/evidence", "/v1/tax/suppliers/{supplier_id}/evidence"):
    if path not in openapi:
        raise SystemExit(f"FAIL: rota de contrato ausente: {path}")
for invented in ("tax_rate", "tax_code", "risk_score", "compliance_status", "purchase_order_create"):
    if invented in openapi:
        raise SystemExit(f"FAIL: campo/regra não aprovado entrou no contrato: {invented}")

print("PASS: Providers residem somente na Application; Core/Functional Core/participants permanecem sem HTTP e infraestrutura.")
print("PASS: contratos síncronos, read-only, tipados e Fakes feature-gated.")
print("PASS: OpenAPI contém somente referências externas e proveniência; nenhuma regra fiscal/compliance foi inventada.")
PY
