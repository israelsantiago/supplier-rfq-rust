#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

CAPABILITIES_DIR="src/capabilities"
HANDLERS="src/http/handlers.rs"

[[ -d "$CAPABILITIES_DIR" ]] || {
  echo "ERRO: diretório de capabilities não encontrado."
  exit 1
}
[[ -f "$HANDLERS" ]] || {
  echo "ERRO: handlers HTTP não encontrados."
  exit 1
}

fail() {
  echo "FALHA: $1" >&2
  exit 1
}

echo "== 1. Capabilities não devem depender de Axum =="

if rg -n '(^use axum::|axum::)' "$CAPABILITIES_DIR"; then
  fail "foi encontrada dependência HTTP dentro de capabilities"
else
  echo "OK"
fi

echo
echo "== 2. Um RPU não deve depender de outro RPU =="

python3 - <<'PY_CHECK'
import re
import sys
from pathlib import Path

cap_dir = Path("src/capabilities")

if not cap_dir.is_dir():
    print(f"ERRO: diretório não encontrado: {cap_dir}", file=sys.stderr)
    sys.exit(1)

# Descobre os nomes dos módulos de capabilities existentes no checkout.
capability_names = set()

for child in cap_dir.iterdir():
    if child.is_file() and child.suffix == ".rs" and child.stem != "mod":
        capability_names.add(child.stem)
    elif child.is_dir() and any(child.rglob("*.rs")):
        capability_names.add(child.name)

if not capability_names:
    print("ERRO: nenhuma capability identificada.", file=sys.stderr)
    sys.exit(1)

findings = set()

for source in sorted(cap_dir.rglob("*.rs")):
    relative = source.relative_to(cap_dir)

    # Arquivo raiz da capability: open_rfq.rs.
    # Arquivos internos: open_rfq/decision.rs, open_rfq/request.rs etc.
    if len(relative.parts) == 1:
        owner = source.stem if source.stem in capability_names else None
    else:
        owner = relative.parts[0] if relative.parts[0] in capability_names else None

    text = source.read_text()
    import_spans = []

    # Inspeciona imports, inclusive os que usam chaves e várias linhas.
    for match in re.finditer(
        r"\buse\s+crate::capabilities::(.*?);",
        text,
        re.DOTALL,
    ):
        import_spans.append((match.start(), match.end()))
        imported_names = set(
            re.findall(r"\b[A-Za-z_]\w*\b", match.group(1))
        ) & capability_names

        for referenced in imported_names:
            if referenced != owner:
                findings.add(
                    (source.as_posix(), owner or "<raiz>", referenced)
                )

    # Também detecta caminhos absolutos qualificados usados fora de `use`.
    for match in re.finditer(
        r"\bcrate::capabilities::([A-Za-z_]\w*)",
        text,
    ):
        if any(
            start <= match.start() < end
            for start, end in import_spans
        ):
            continue

        referenced = match.group(1)

        if referenced in capability_names and referenced != owner:
            findings.add(
                (source.as_posix(), owner or "<raiz>", referenced)
            )

if findings:
    print("FALHA: referências explícitas entre capabilities:")
    for source, owner, referenced in sorted(findings):
        print(
            f"  {source}: capability proprietária={owner}; "
            f"referência externa={referenced}"
        )
    sys.exit(1)

print("OK: nenhuma referência cruzada explícita entre capabilities.")
print("Imports de módulos internos da própria capability são permitidos.")
PY_CHECK

echo
echo "== 3. Handlers devem desserializar DTOs HTTP, não requests internos =="

if rg -n \
  'Json\(request\):[[:space:]]*Json<(OpenRfqRequest|RegisterSupplierRequest|SetSupplierEligibilityRequest|SubmitSupplierQuoteRequest)>' \
  "$HANDLERS"; then
  fail "um handler ainda desserializa diretamente um request interno"
else
  echo "OK"
fi

echo
echo "== 4. Revisão informativa: dependências técnicas nos módulos de decisão/contexto =="

PURE_CANDIDATES=()
while IFS= read -r file; do
  PURE_CANDIDATES+=("$file")
done < <(
  find "$CAPABILITIES_DIR" -type f \
    \( -name 'decision.rs' -o -name 'generate_consequences.rs' \) \
    | sort
)

if ((${#PURE_CANDIDATES[@]})); then
  if rg -n \
    '(^use factstr::|^use sqlx::|^use axum::|EventStore|EventQuery|QueryResult|PgPool)' \
    "${PURE_CANDIDATES[@]}"; then
    echo
    echo "REVISÃO MANUAL: avalie se os tipos técnicos encontrados são apenas"
    echo "dados/contexto necessários ou se podem ficar no shell imperativo."
    echo "Este relatório não falha automaticamente: o exemplo oficial Rust"
    echo "também permite tipos FACTSTR no contexto e no shell de um RPU."
  else
    echo "Nenhuma referência técnica óbvia encontrada nos arquivos analisados."
  fi
else
  echo "Nenhum módulo candidato encontrado; revise a estrutura dos RPUs."
fi

echo
echo "== 5. Feature slices e contratos compartilhados =="

[[ -f src/events/mod.rs ]] || fail "módulo compartilhado src/events/mod.rs não encontrado"
[[ ! -e src/facts.rs ]] || fail "src/facts.rs antigo ainda existe; contratos devem estar em src/events/"

FEATURES=(
  close_rfq
  get_rfq_summary
  list_rfq_summaries
  list_supplier_quotes
  open_rfq
  register_supplier
  set_supplier_eligibility
  submit_supplier_quote
)

for feature in "${FEATURES[@]}"; do
  [[ -f "$CAPABILITIES_DIR/$feature/mod.rs" ]] || fail "feature '$feature' precisa ter seu próprio mod.rs"
  [[ -f "$CAPABILITIES_DIR/$feature/process_request.rs" ]] || fail "feature '$feature' precisa ter process_request.rs"
done

echo "OK: os oito RPUs possuem pasta de feature e process_request.rs."

echo
if [[ "${SKIP_RUST_VALIDATION:-0}" == "1" ]]; then
  echo "Validação estrutural executada; os comandos Cargo foram executados pela etapa chamadora."
else
  echo "== 6. Validação Rust =="
  cargo fmt --all -- --check
  cargo check --lib --all-features --locked
  cargo check --all-targets --all-features --locked
  cargo test --all-targets --all-features --locked
  cargo clippy --all-targets --all-features --locked -- -D warnings
  git diff --check
fi

echo
bash scripts/verify-application-state-boundary.sh

echo "Verificações concluídas."
