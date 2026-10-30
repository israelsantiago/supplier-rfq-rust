#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
python3 - <<'PY_CHECK'
from pathlib import Path
from urllib.parse import unquote, urlsplit
import re
import sys

root = Path.cwd()
readme_path = Path("README.md")
design_path = Path("docs/design-approach.md")
flow_path = Path("docs/flows/atomic-rfq-sourcing.md")
adr_index_path = Path("docs/architecture/adr/README.md")
adr31_path = Path("docs/architecture/adr/0031-atomic-rfq-sourcing-finalization.md")
adr33_path = Path("docs/architecture/adr/0033-application-state-provider-specialized-ports.md")

for required in (readme_path, design_path, flow_path, adr_index_path, adr31_path, adr33_path):
    if not required.is_file():
        raise SystemExit(f"ERRO: documentação obrigatória ausente: {required}")

readme = readme_path.read_text(encoding="utf-8")
design = design_path.read_text(encoding="utf-8")
flow = flow_path.read_text(encoding="utf-8")
adr_index = adr_index_path.read_text(encoding="utf-8")
adr31 = adr31_path.read_text(encoding="utf-8")
adr33 = adr33_path.read_text(encoding="utf-8")
adr26 = Path("docs/architecture/adr/0026-application-state-provider.md").read_text(encoding="utf-8")
architecture = Path("docs/architecture-event-sourcing-factstr.md").read_text(encoding="utf-8")

# README current-state claims: all command capabilities, including the atomic sourcing command.
readme_required = (
    "FinalizeRfqSourcing",
    "POST /api/rfqs/{rfq_id}/finalize-sourcing",
    "rfq_awarded.v1",
    "purchase_order_created.v1",
    "purchase_order_dispatch_requested.v1",
    "scripts/verify-rpu-boundaries.sh",
    "scripts/verify-openapi.sh",
    "scripts/verify-mermaid-diagrams.sh",
    "scripts/verify-documentation-current-state.sh",
    "ADR-0033",
)
for token in readme_required:
    if token not in readme:
        raise SystemExit(f"ERRO: README não documenta o estado atual obrigatório: {token}")
for stale in ("scripts/run-all.sh", "docs/PLANO-E-CRITERIOS-DE-ACEITE.md", "not yet merged into main", "proposta de implementação aditiva"):
    if stale in readme:
        raise SystemExit(f"ERRO: referência obsoleta no README: {stale}")

# The guide must distinguish implemented HTTP idempotency from remaining operational work.
if "idempotência HTTP está implementada" not in design:
    raise SystemExit("ERRO: guia de design ainda não reconhece a implementação de idempotência HTTP.")
if "FinalizeRfqSourcing" not in design or "não é um Reactor" not in design:
    raise SystemExit("ERRO: guia de design não distingue a RPU de sourcing de um Reactor.")
if "FinalizeRfqSourcing" not in architecture or "AtomicSourcingStateProvider" not in architecture:
    raise SystemExit("ERRO: guia FACTSTR não documenta a capability/porta especializada.")

# Implemented status must match the published main branch.
if not re.search(r"(?im)^- \*\*Status:\*\* Accepted; implementado na branch `main`", adr31):
    raise SystemExit("ERRO: ADR-0031 não está marcada como implementada na main.")
if "not yet merged into main" in adr31.lower() or "proposta de implementação" in flow.lower():
    raise SystemExit("ERRO: ADR-0031/fluxo ainda contém status anterior à implementação.")
if "não é um Reactor" not in adr31 or "fachada" not in adr31.lower():
    raise SystemExit("ERRO: ADR-0031 não explica a classificação de AtomicSourcingFlow.")
if "AtomicSourcingStateProvider" not in adr26 or "FactstrAtomicSourcingStateProvider" not in adr26:
    raise SystemExit("ERRO: ADR-0026 não documenta o adaptador especializado.")
if "mesma instância compartilhada de `PostgresStore`" not in adr33:
    raise SystemExit("ERRO: ADR-0033 não esclarece a fonte autoritativa compartilhada.")

# All numbered ADRs must be linked from the index.
adr_dir = Path("docs/architecture/adr")
adr_files = sorted(adr_dir.glob("[0-9][0-9][0-9][0-9]-*.md"))
if not adr_files:
    raise SystemExit("ERRO: nenhum ADR numerado encontrado.")
for path in adr_files:
    if path.name not in adr_index:
        raise SystemExit(f"ERRO: ADR não indexado: {path.name}")

# Validate all repository-relative inline Markdown links, including images.
# Fenced code blocks are removed first so examples are not treated as documentation links.
link_pattern = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
issues = []
markdown_files = [
    Path("README.md"),
    Path("docs/design-approach.md"),
    Path("docs/architecture-event-sourcing-factstr.md"),
    Path("docs/architecture/rpu-capability-boundaries.md"),
    Path("docs/flows/atomic-rfq-sourcing.md"),
    Path("docs/testing/application-state-provider.md"),
    *sorted(adr_dir.glob("*.md")),
]
for md in markdown_files:
    if not md.is_file():
        issues.append(f"{md}: arquivo documental do escopo ausente")
        continue
    try:
        source = md.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
        continue
    source = re.sub(r"(?ms)^```[^\n]*\n.*?^```\s*$", "", source)
    source = re.sub(r"(?ms)^~~~[^\n]*\n.*?^~~~\s*$", "", source)
    for match in link_pattern.finditer(source):
        raw = match.group(1).strip()
        if raw.startswith("<") and ">" in raw:
            raw = raw[1:raw.index(">")]
        else:
            # Optional Markdown title follows the destination, separated by whitespace.
            raw = raw.split()[0] if raw else ""
        if not raw:
            continue
        parsed = urlsplit(raw)
        if parsed.scheme or raw.startswith("//") or not parsed.path:
            continue
        target = Path(unquote(parsed.path))
        resolved = target if target.is_absolute() else (md.parent / target)
        if not resolved.exists():
            line = source.count("\n", 0, match.start()) + 1
            issues.append(f"{md}:{line}: destino inexistente: {raw}")
if issues:
    print("ERRO: links Markdown relativos sem destino:", file=sys.stderr)
    print("\n".join(issues), file=sys.stderr)
    raise SystemExit(1)

# The repository uses main as the sole active delivery branch.
ci_path = Path(".github/workflows/ci.yml")
if not ci_path.is_file():
    raise SystemExit("ERRO: workflow .github/workflows/ci.yml ausente.")
ci = ci_path.read_text(encoding="utf-8")
branches = re.findall(r"branches:\s*\[([^\]]+)\]", ci)
if len(branches) < 2 or any(value.strip() != "main" for value in branches):
    raise SystemExit(f"ERRO: filtros de push/PR devem apontar somente para main; encontrados: {branches}")

print(f"OK: README, {len(adr_files)} ADRs indexadas, ADR-0031/0033 e arquitetura FACTSTR alinhados.")
print(f"OK: links Markdown relativos verificados em {len(markdown_files)} arquivos .md.")
print("OK: capabilities, idempotência HTTP, classificação RPU/Reactor e branch main consistentes.")
PY_CHECK
