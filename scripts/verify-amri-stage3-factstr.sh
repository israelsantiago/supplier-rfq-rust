#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
[[ "$(git branch --show-current)" == "feat/amri-stage3-composite-context" ]] || { echo "ERRO: branch Stage 3 incorreta." >&2; exit 2; }
# Mutation testing is intentionally deferred to the final AMRI campaign.
for gate in context-verify context-coverage; do
  bash scripts/amri-stage3-gate-state.sh verify "$gate"
done
bash scripts/amri-stage3-gate-state.sh invalidate factstr-integration
python3 - <<'PY'
from pathlib import Path
import re

context = Path("src/application_state/atomic_interaction/context.rs").read_text(encoding="utf-8")
port = Path("src/application_state/atomic_interaction/state_port.rs").read_text(encoding="utf-8")
adapter = Path("src/application_state/atomic_interaction/factstr.rs").read_text(encoding="utf-8")
mod = Path("src/application_state/atomic_interaction/mod.rs").read_text(encoding="utf-8")
test = Path("tests/amri_factstr_state.rs").read_text(encoding="utf-8")
if re.search(r"\b(?:EventQuery|EventFilter|EventStore|PostgresStore|factstr)\b", context + port):
    raise SystemExit("FALHA: tipos FACTSTR vazaram para contexto/porta pública.")
if mod.count("pub mod factstr;") != 1:
    raise SystemExit("FALHA: adapter FACTSTR não está exportado exatamente uma vez.")
if len(re.findall(r"\.query\s*\(", adapter)) != 1:
    raise SystemExit("FALHA: esperado exatamente um query no adapter FACTSTR.")
if len(re.findall(r"\.append_if\s*\(", adapter)) != 1:
    raise SystemExit("FALHA: esperado exatamente um append_if no adapter FACTSTR.")
if "current_context_version" not in adapter or "token.context_query" not in adapter or "token.expected_context_version" not in adapter:
    raise SystemExit("FALHA: adapter não está preservando e reutilizando query/version retornadas pelo mesmo read.")
if re.search(r"max\s*\(", adapter):
    raise SystemExit("FALHA: adapter não pode compor versões independentes usando max().")
if "Postgres::default()" not in test or "Barrier::new(2)" not in test or "testcontainers_modules" not in test:
    raise SystemExit("FALHA: integração precisa comprovar Postgres real e corrida de conexões independentes.")
print("PASS: FACTSTR está isolado no adapter; uma query e um append_if; token usa query/context version originais.")
print("PASS: teste de integração cobre PostgreSQL/Testcontainers e concorrência com conexões independentes.")
PY
cargo fmt --all -- --check
cargo check --lib --no-default-features --locked
printf '\n== Unit tests do adapter sem executar Testcontainers ==\n'
DOCKER_HOST="unix:///tmp/amri-stage3-adapter-unit-must-not-use-docker.sock" \
  cargo test --lib --no-default-features --locked application_state::atomic_interaction::factstr:: -- --test-threads=1
printf '\n== Clippy do target de integração (compila; não executa containers) ==\n'
cargo clippy --test amri_factstr_state --no-default-features --locked -- -D warnings
printf '\n== FACTSTR/PostgreSQL integration proof; Docker is intentionally required ==\n'
cargo test --test amri_factstr_state --no-default-features --locked -- --test-threads=1
git diff --check
bash scripts/amri-stage3-gate-state.sh record factstr-integration
echo "PASS: FACTSTR 0.5.2 composite-context adapter integration tests concluídos."
