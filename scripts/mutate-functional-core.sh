#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

bash scripts/verify-functional-core-test-isolation.sh

# Put cargo-mutants scratch trees on the checkout filesystem instead of a possibly
# small /tmp tmpfs. The latter can cause ld/lld to fail with SIGBUS while mapping output.
MUTANTS_TMPDIR="${MUTANTS_TMPDIR:-$ROOT/target/cargo-mutants-tmp}"
mkdir -p "$MUTANTS_TMPDIR"
MUTANTS_TMPDIR="$(cd "$MUTANTS_TMPDIR" && pwd -P)"
export MUTANTS_TMPDIR
export TMPDIR="$MUTANTS_TMPDIR"

PROFILE_OUTPUT="$(python3 scripts/mutation-resource-profile.py --shell --scratch "$MUTANTS_TMPDIR")" || {
  echo "ERRO: não foi possível calcular um perfil seguro de recursos para mutation testing." >&2
  echo "Use MUTANTS_TMPDIR para apontar para outro filesystem com espaço livre suficiente." >&2
  df -h "$MUTANTS_TMPDIR" 2>&1 || true
  free -h 2>&1 || true
  exit 2
}
CPU_COUNT= AFFINITY_CPU_COUNT= PHYSICAL_CORE_COUNT= MEMORY_GIB= DISK_GIB= MUTANTS_JOBS_SELECTED= CARGO_BUILD_JOBS_SELECTED= MUTANTS_SAFE_JOB_LIMIT= MUTANTS_JOBS_CLAMPED=
while IFS='=' read -r key value; do
  case "$key" in
    CPU_COUNT) CPU_COUNT="$value" ;;
    AFFINITY_CPU_COUNT) AFFINITY_CPU_COUNT="$value" ;;
    PHYSICAL_CORE_COUNT) PHYSICAL_CORE_COUNT="$value" ;;
    MEMORY_GIB) MEMORY_GIB="$value" ;;
    DISK_GIB) DISK_GIB="$value" ;;
    MUTANTS_JOBS_SELECTED) MUTANTS_JOBS_SELECTED="$value" ;;
    CARGO_BUILD_JOBS_SELECTED) CARGO_BUILD_JOBS_SELECTED="$value" ;;
    MUTANTS_SAFE_JOB_LIMIT) MUTANTS_SAFE_JOB_LIMIT="$value" ;;
    MUTANTS_JOBS_CLAMPED) MUTANTS_JOBS_CLAMPED="$value" ;;
    *) echo "ERRO: campo inesperado do perfil de recursos: $key" >&2; exit 2 ;;
  esac
done <<< "$PROFILE_OUTPUT"
for variable in CPU_COUNT AFFINITY_CPU_COUNT PHYSICAL_CORE_COUNT MEMORY_GIB DISK_GIB MUTANTS_JOBS_SELECTED CARGO_BUILD_JOBS_SELECTED MUTANTS_SAFE_JOB_LIMIT; do
  [[ "${!variable}" =~ ^[0-9]+$ ]] || { echo "ERRO: perfil de recursos incompleto: $variable" >&2; exit 2; }
done

MUTANTS_TIMEOUT="${MUTANTS_TIMEOUT:-90}"
[[ "$MUTANTS_TIMEOUT" =~ ^[1-9][0-9]*$ ]] || { echo "MUTANTS_TIMEOUT deve ser inteiro positivo." >&2; exit 2; }
if [[ "$MUTANTS_JOBS_CLAMPED" == "True" ]]; then
  echo "AVISO: MUTANTS_JOBS solicitado foi limitado ao teto seguro de $MUTANTS_SAFE_JOB_LIMIT workers."
fi
MUTANTS_JOBS="$MUTANTS_JOBS_SELECTED"
CARGO_BUILD_JOBS="$CARGO_BUILD_JOBS_SELECTED"
# Debug symbols increase linker memory, scratch-disk usage, and compile/link time.
# This setting affects this mutation run only; the full-regression profile is unchanged.
MUTANTS_DEBUG_INFO="${MUTANTS_DEBUG_INFO:-0}"
[[ "$MUTANTS_DEBUG_INFO" =~ ^[012]$ ]] || { echo "MUTANTS_DEBUG_INFO deve ser 0, 1 ou 2." >&2; exit 2; }
export CARGO_BUILD_JOBS
export CARGO_PROFILE_TEST_DEBUG="$MUTANTS_DEBUG_INFO"
export CARGO_PROFILE_DEV_DEBUG="$MUTANTS_DEBUG_INFO"

if ! cargo mutants --version >/dev/null 2>&1; then
  echo "== cargo-mutants não está instalado; instalando agora =="
  cargo install --locked --force cargo-mutants || {
    echo "ERRO: falha ao instalar cargo-mutants. Verifique a conectividade e execute: cargo install --locked --force cargo-mutants" >&2
    exit 2
  }
fi
cargo mutants --version >/dev/null 2>&1 || {
  echo "ERRO: cargo-mutants continua indisponível após a instalação." >&2
  exit 2
}

printf '\n== Perfil dinâmico do mutation testing ==\n'
printf 'Núcleos físicos/workers efetivamente disponíveis: %s; núcleos físicos detectados: %s; CPUs lógicas na afinidade: %s\n' "$CPU_COUNT" "$PHYSICAL_CORE_COUNT" "$AFFINITY_CPU_COUNT"
printf 'Memória disponível estimada: %s GiB; espaço livre no filesystem scratch: %s GiB\n' "$MEMORY_GIB" "$DISK_GIB"
printf 'Workers cargo-mutants: %s; CARGO_BUILD_JOBS por worker: %s; debug symbols: %s\n' "$MUTANTS_JOBS" "$CARGO_BUILD_JOBS" "$MUTANTS_DEBUG_INFO"
printf 'TMPDIR: %s\n' "$TMPDIR"
df -h "$TMPDIR" || true

RUN_LOG="$ROOT/target/mutation-testing/cargo-mutants-$(date -u +%Y%m%dT%H%M%SZ).log"
BASELINE_LOG="$ROOT/target/mutation-testing/functional-core-baseline-$(date -u +%Y%m%dT%H%M%SZ).log"
LIST_LOG="$ROOT/target/mutation-testing/functional-core-tests-list-$(date -u +%Y%m%dT%H%M%SZ).log"
mkdir -p "$(dirname "$RUN_LOG")"
FILTERS=(
  'application_state::factstr::tests::provider_reads_typed_facts_and_rejects_stale_conditional_append'
  'set_supplier_eligibility::load_context::context_scope_tests::eligibility_history_does_not_change_assignment_context_version'
  'submit_supplier_quote::process_request::ccc_tests::ccc_two_independent_stores_from_same_snapshot_commit_exactly_one_quote_fact'
  'submit_supplier_quote::process_request::ccc_tests::ccc_supplier_eligibility_change_invalidates_a_prepared_quote'
  'submit_supplier_quote::process_request::ccc_tests::ccc_independent_supplier_contexts_commit_without_false_conflict'
  'http::idempotency::tests::ledger_state_machine_fails_closed_and_expires_only_completed_records'
  'http::idempotency::tests::oversized_response_marks_key_unknown_and_blocks_retry'
  'projections::rfq_summary_projection::tests::replaying_the_same_fact_batch_does_not_double_count_quotes'
  'projections::rfq_summary_projection::tests::failed_projection_batch_rolls_back_earlier_events'
)
TEST_ARGS=()
for filter in "${FILTERS[@]}"; do TEST_ARGS+=(--skip "$filter"); done

# Confirm filters and run the unmutated library baseline in the original target cache.
# This safely enables --baseline=skip in cargo-mutants: the duplicate scratch-tree baseline
# would rebuild and test the same library again, with extra temporary disk consumption.
echo "== Validando filtros de storage antes da mutation testing =="
set +e
DOCKER_HOST="unix:///tmp/supplier-rfq-functional-core-must-not-use-docker.sock" cargo test --lib --all-features --locked -- --list 2>&1 | tee "$LIST_LOG"
list_rc=${PIPESTATUS[0]}; set -e
if [[ "$list_rc" -ne 0 ]]; then echo "FALHA ao listar testes da library; log: $LIST_LOG" >&2; exit "$list_rc"; fi
for filter in "${FILTERS[@]}"; do
  if ! grep -Fq "$filter" "$LIST_LOG"; then echo "FALHA: filtro não corresponde a um teste listado: $filter" >&2; exit 2; fi
done
echo "== Baseline puro: cargo test --lib, com os nove testes de storage excluídos =="
set +e
DOCKER_HOST="unix:///tmp/supplier-rfq-functional-core-must-not-use-docker.sock" cargo test --lib --all-features --locked -- "${TEST_ARGS[@]}" --test-threads=1 2>&1 | tee "$BASELINE_LOG"
baseline_rc=${PIPESTATUS[0]}; set -e
if [[ "$baseline_rc" -ne 0 ]]; then echo "FALHA: baseline da library sem Docker; log: $BASELINE_LOG" >&2; exit "$baseline_rc"; fi

MUTANTS_CMD=(
  cargo mutants
  --all-features
  --baseline=skip
  --no-shuffle
  --timeout "$MUTANTS_TIMEOUT"
  --jobs "$MUTANTS_JOBS"
  --file 'src/capabilities/*/generate_consequences.rs'
  --file 'src/capabilities/*/build_context.rs'
  --file 'src/capabilities/submit_supplier_quote/decision.rs'
  --file 'src/capabilities/finalize_rfq_sourcing/decision.rs'
  --cargo-test-arg=--lib
  --
  --
)
for filter in "${FILTERS[@]}"; do MUTANTS_CMD+=(--skip "$filter"); done
MUTANTS_CMD+=(--test-threads=1)

set +e
DOCKER_HOST="unix:///tmp/supplier-rfq-functional-core-must-not-use-docker.sock" "${MUTANTS_CMD[@]}" 2>&1 | tee "$RUN_LOG"
rc=${PIPESTATUS[0]}
set -e
if [[ "$rc" -ne 0 ]]; then
  echo "FALHA: cargo-mutants terminou com código $rc. Log: $RUN_LOG" >&2
  echo "== Diagnóstico de recursos após falha ==" >&2
  df -h "$TMPDIR" >&2 || true
  df -i "$TMPDIR" >&2 || true
  free -h >&2 || true
  if grep -Eiq 'signal 7|Bus error|No space left on device|disk quota exceeded' "$RUN_LOG"; then
    echo "ALERTA: erro compatível com pressão de armazenamento/linker ou recursos; confira df/free acima. O runner já moveu TMPDIR para o filesystem do checkout." >&2
  fi
  exit "$rc"
fi

cat <<MSG

Mutation run terminou. Revise NOT CAUGHT/MISSED e TIMEOUT em mutants.out/ e no log $RUN_LOG.
Uma mutação não capturada não é automaticamente defeito: revise a semântica e acrescente
assertions quando o comportamento for relevante. Mutantes unviable não provam cobertura.
Perfil detectado: $CPU_COUNT CPU(s), $MEMORY_GIB GiB de RAM disponível, $DISK_GIB GiB livres;
$MUTANTS_JOBS workers externos e $CARGO_BUILD_JOBS jobs Cargo por worker. O runner não deve
iniciar Docker/Testcontainers; esses testes ficam na suíte integral de regressão.
MSG
