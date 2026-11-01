#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

bash scripts/amri-stage2-gate-state.sh invalidate mutation

bash scripts/verify-amri-core-boundaries.sh

MUTANTS_TMPDIR="${MUTANTS_TMPDIR:-$ROOT/target/cargo-mutants-amri-core-tmp}"
mkdir -p "$MUTANTS_TMPDIR"
MUTANTS_TMPDIR="$(cd "$MUTANTS_TMPDIR" && pwd -P)"
export MUTANTS_TMPDIR TMPDIR="$MUTANTS_TMPDIR"

if ! cargo mutants --version >/dev/null 2>&1; then
  cargo install --locked cargo-mutants || {
    echo "ERRO: instalação de cargo-mutants falhou." >&2
    exit 2
  }
fi

MUTANTS_TIMEOUT="${MUTANTS_TIMEOUT:-90}"
[[ "$MUTANTS_TIMEOUT" =~ ^[1-9][0-9]*$ ]] || { echo "MUTANTS_TIMEOUT deve ser inteiro positivo." >&2; exit 2; }
MUTANTS_JOBS="${MUTANTS_JOBS:-1}"
[[ "$MUTANTS_JOBS" =~ ^[1-9][0-9]*$ ]] || { echo "MUTANTS_JOBS deve ser inteiro positivo." >&2; exit 2; }

printf '\n== Baseline sem Docker: somente testes unitários AMRI ==\n'
DOCKER_HOST="unix:///tmp/amri-core-must-not-use-docker.sock" \
  cargo test --lib --no-default-features --locked amri:: -- --test-threads=1

printf '\n== Mutation testing restrito aos módulos de produção src/amri ==\n'
RUN_LOG="$ROOT/target/amri-stage2/mutation-amri-core-$(date -u +%Y%m%dT%H%M%SZ).log"
mkdir -p "$(dirname "$RUN_LOG")"
MUTANTS_CMD=(
  cargo mutants
  --no-default-features
  --baseline=skip
  --no-shuffle
  --timeout "$MUTANTS_TIMEOUT"
  --jobs "$MUTANTS_JOBS"
  # Equivalent mutant: high and low hexadecimal nibbles occupy disjoint bit ranges.
  --exclude-re
  '^src/amri/identity\.rs:[0-9]+:[0-9]+: replace \| with \^ in Fingerprint::from_hex$'
  --cargo-test-arg=--lib
  --file src/amri/identity.rs
  --file src/amri/definition.rs
  --file src/amri/execution.rs
  --file src/amri/outcome.rs
  --file src/amri/plan.rs
  --file src/amri/error.rs
  --
  amri::
  --
  --test-threads=1
)

set +e
DOCKER_HOST="unix:///tmp/amri-core-must-not-use-docker.sock" "${MUTANTS_CMD[@]}" 2>&1 | tee "$RUN_LOG"
rc=${PIPESTATUS[0]}
set -e
if [[ "$rc" -ne 0 ]]; then
  echo "FALHA: cargo-mutants terminou com código $rc. Log: $RUN_LOG" >&2
  echo "Este runner não executa integration tests e não inicia Testcontainers." >&2
  exit "$rc"
fi

printf 'PASS: mutation testing AMRI Core concluído. Log: %s\n' "$RUN_LOG"
bash scripts/amri-stage2-gate-state.sh record mutation
