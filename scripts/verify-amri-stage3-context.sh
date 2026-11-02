#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
[[ "$(git branch --show-current)" == "feat/amri-stage3-composite-context" ]] || { echo "ERRO: branch Stage 3 incorreta." >&2; exit 2; }
bash scripts/amri-stage3-gate-state.sh invalidate context-verify

printf '\n== 0. Provar que o fake/planner não importa backend ou infraestrutura ==\n'
python3 - <<'PY'
from pathlib import Path
import re

files = [
    Path("src/application_state/atomic_interaction/context.rs"),
    Path("src/application_state/atomic_interaction/state_port.rs"),
    Path("src/application_state/atomic_interaction/fake.rs"),
]
for path in files:
    source = path.read_text(encoding="utf-8")
    if re.search(r"\b(?:factstr|factstr_postgres|EventQuery|EventFilter|EventStore|PostgresStore|sqlx|tokio|testcontainers|ContainerAsync)\b", source):
        raise SystemExit(f"FALHA: backend/runtime/infra referenciado em {path}")
mod = Path("src/application_state/atomic_interaction/mod.rs").read_text(encoding="utf-8")
if "pub mod context;" not in mod or "pub mod state_port;" not in mod or "#[cfg(test)]\nmod fake;" not in mod:
    raise SystemExit("FALHA: fake deve permanecer sob cfg(test), e os contratos devem estar exportados.")
for path in ("Cargo.toml", "Cargo.lock"):
    result = __import__("subprocess").run(["git", "diff", "--quiet", "HEAD", "--", path])
    if result.returncode != 0:
        raise SystemExit(f"FALHA: Stage 3 inesperadamente alterou {path}")
print("PASS: Stage 3A usa contratos application-owned; nenhum módulo do fake/planner importa FACTSTR, SQLx ou Testcontainers.")
print("PASS: fake está exclusivamente sob cfg(test); Cargo.toml/Cargo.lock permanecem inalterados.")
PY

printf '\n== 1. Revalidar Core puro da Stage 2 ==\n'
bash scripts/verify-amri-core.sh

printf '\n== 2. Formatação ==\n'
cargo fmt --all -- --check

printf '\n== 3. Check library sem features ==\n'
cargo check --lib --no-default-features --locked

printf '\n== 4. Testes puros do planner/fake; nenhum Testcontainers ==\n'
DOCKER_HOST="unix:///tmp/amri-stage3-fake-must-not-use-docker.sock" \
  cargo test --lib --no-default-features --locked application_state::atomic_interaction:: -- --test-threads=1

printf '\n== 5. Clippy library com testes unitários ==\n'
cargo clippy --lib --no-default-features --locked -- -D warnings

git diff --check
bash scripts/amri-stage3-gate-state.sh record context-verify
echo "PASS: Stage 3A context contract/fake verify concluído sem iniciar Docker/Testcontainers."
