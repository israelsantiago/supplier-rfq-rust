#!/usr/bin/env bash
# DEFERRED POLICY: do not execute during intermediate AMRI stages.
# Run the consolidated mutation campaign once, after the entire AMRI implementation,
# against eligible pure Functional Core logic only; no PostgreSQL/Testcontainers.
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
bash scripts/amri-stage3-gate-state.sh invalidate context-mutation
bash scripts/verify-amri-stage3-context.sh

if ! cargo mutants --version >/dev/null 2>&1; then
  cargo install --locked cargo-mutants
fi
MUTANTS_TIMEOUT="${MUTANTS_TIMEOUT:-90}"
MUTANTS_JOBS="${MUTANTS_JOBS:-1}"
[[ "$MUTANTS_TIMEOUT" =~ ^[1-9][0-9]*$ ]] || { echo "ERRO: MUTANTS_TIMEOUT deve ser positivo." >&2; exit 2; }
[[ "$MUTANTS_JOBS" =~ ^[1-9][0-9]*$ ]] || { echo "ERRO: MUTANTS_JOBS deve ser positivo." >&2; exit 2; }
RUN_LOG="$ROOT/target/amri-stage3/mutation-context-$(date -u +%Y%m%dT%H%M%SZ).log"
mkdir -p "$(dirname "$RUN_LOG")"
MUTANTS_CMD=(
  cargo mutants
  --no-default-features
  --baseline=skip
  --no-shuffle
  --timeout "$MUTANTS_TIMEOUT"
  --jobs "$MUTANTS_JOBS"
  --file src/application_state/atomic_interaction/context.rs
  --file src/application_state/atomic_interaction/state_port.rs
  --cargo-test-arg=--lib
  --
  application_state::atomic_interaction::
  --
  --test-threads=1
)
printf '\n== Mutation testing somente context/state_port; fake unit tests ==\n'
set +e
DOCKER_HOST="unix:///tmp/amri-stage3-fake-must-not-use-docker.sock" "${MUTANTS_CMD[@]}" 2>&1 | tee "$RUN_LOG"
rc=${PIPESTATUS[0]}
set -e
if [[ "$rc" -ne 0 ]]; then
  echo "FALHA: mutation Stage 3A terminou com código $rc. Log: $RUN_LOG" >&2
  echo "Este runner não executa integration tests e não inicia Testcontainers." >&2
  exit "$rc"
fi
echo "PASS: mutation testing Stage 3A concluído. Log: $RUN_LOG"
bash scripts/amri-stage3-gate-state.sh record context-mutation
