#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
CAPABILITIES="src/capabilities"
[[ -d "$CAPABILITIES" ]] || { echo "ERRO: $CAPABILITIES não existe." >&2; exit 2; }

python3 - <<'PY'
from pathlib import Path
import re, sys
capabilities = Path("src/capabilities")
violations = []
for path in sorted(capabilities.rglob("*.rs")):
    if path.name == "ccc_tests.rs":
        continue
    source = path.read_text(encoding="utf-8")
    # Test-only modules may seed/query FACTSTR directly; the production shell before #[cfg(test)] may not.
    match = re.search(r"^\s*#\[cfg\(test\)\]", source, re.MULTILINE)
    production = source[:match.start()] if match else source
    checks = [
        (r"^\s*use\s+factstr(?:\s*::|\s*;)", "import direto de factstr"),
        (r"\b(?:EventStore|EventQuery|EventFilter|EventRecord|NewEvent|EventStoreError|QueryResult)\b", "tipo/operação FACTSTR"),
        (r"\bfactstr_postgres\b|\bPostgresStore\b|\bPgPool\b", "implementação concreta de storage"),
    ]
    for pattern, label in checks:
        for found in re.finditer(pattern, production, re.MULTILINE):
            line = production.count("\n", 0, found.start()) + 1
            violations.append(f"{path}:{line}: {label}")
if violations:
    print("FALHA: a fronteira Application State foi violada:", file=sys.stderr)
    print("\n".join(violations), file=sys.stderr)
    sys.exit(1)

adapter = Path("src/application_state/factstr.rs")
if not adapter.is_file():
    raise SystemExit("ERRO: adaptador FactstrApplicationStateProvider ausente.")
text = adapter.read_text(encoding="utf-8")
for required in ["EventQuery", "EventFilter", "NewEvent", "append_if", "ConditionalAppendConflict"]:
    if required not in text:
        raise SystemExit(f"ERRO: adaptador FACTSTR não contém o contrato esperado: {required}")
facade = Path("src/app/command_rpus.rs").read_text(encoding="utf-8")
if "ApplicationStateProvider" not in facade or "EventStore" in facade or "PostgresStore" in facade:
    raise SystemExit("ERRO: CommandRpus ainda conhece a tecnologia de storage em vez da porta ASP.")
handlers = Path("src/http/handlers.rs").read_text(encoding="utf-8")
if "state.event_store" in handlers:
    raise SystemExit("ERRO: handler HTTP ainda acessa o EventStore bruto.")
legacy = Path("migrations/202610270001_http_idempotency.sql")
dedicated = Path("migrations_idempotency/202610270001_http_idempotency.sql")
if not legacy.is_file() or not dedicated.is_file():
    raise SystemExit("ERRO: migration histórica ou dedicada do ledger ausente.")
if legacy.read_bytes() != dedicated.read_bytes():
    raise SystemExit("ERRO: migration do ledger diverge da cópia histórica preservada.")
print("OK: Application State boundary e migration HTTP histórica/dedicada.")
PY
