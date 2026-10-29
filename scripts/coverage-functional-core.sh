#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

bash scripts/verify-functional-core-test-isolation.sh

command -v rustup >/dev/null 2>&1 || { echo "ERRO: rustup não encontrado; instale Rust via rustup." >&2; exit 2; }
if ! rustup run nightly rustc --version >/dev/null 2>&1; then
  echo "== Instalando Rust nightly + llvm-tools-preview =="
  rustup toolchain install nightly --component llvm-tools-preview || { echo "ERRO: não foi possível instalar nightly; verifique a conectividade." >&2; exit 2; }
else
  rustup component add llvm-tools-preview --toolchain nightly || { echo "ERRO: não foi possível instalar llvm-tools-preview." >&2; exit 2; }
fi
if ! cargo +nightly llvm-cov --version >/dev/null 2>&1; then
  echo "== Instalando cargo-llvm-cov =="
  cargo install --locked --force cargo-llvm-cov || { echo "ERRO: não foi possível instalar cargo-llvm-cov." >&2; exit 2; }
fi
cargo +nightly llvm-cov --version >/dev/null 2>&1 || { echo "ERRO: cargo-llvm-cov continua indisponível." >&2; exit 2; }

mkdir -p target
REPORT="target/functional-core-branch-coverage.json"
HTML_DIR="target/functional-core-branch-coverage-html"

echo "== Functional Core: instrumentação LLVM + branch coverage (nightly) =="
cargo +nightly llvm-cov clean --workspace
DOCKER_HOST="unix:///tmp/supplier-rfq-functional-core-must-not-use-docker.sock" cargo +nightly llvm-cov --branch --lib --json --output-path "$REPORT" -- \
  --skip 'application_state::factstr::tests::provider_reads_typed_facts_and_rejects_stale_conditional_append' \
  --skip 'set_supplier_eligibility::load_context::context_scope_tests::eligibility_history_does_not_change_assignment_context_version' \
  --skip 'submit_supplier_quote::process_request::ccc_tests::ccc_two_independent_stores_from_same_snapshot_commit_exactly_one_quote_fact' \
  --skip 'submit_supplier_quote::process_request::ccc_tests::ccc_supplier_eligibility_change_invalidates_a_prepared_quote' \
  --skip 'submit_supplier_quote::process_request::ccc_tests::ccc_independent_supplier_contexts_commit_without_false_conflict' \
  --skip 'http::idempotency::tests::ledger_state_machine_fails_closed_and_expires_only_completed_records' \
  --skip 'http::idempotency::tests::oversized_response_marks_key_unknown_and_blocks_retry' \
  --skip 'projections::rfq_summary_projection::tests::replaying_the_same_fact_batch_does_not_double_count_quotes' \
  --skip 'projections::rfq_summary_projection::tests::failed_projection_batch_rolls_back_earlier_events' \
  --test-threads=1
set +e
python3 scripts/check-functional-core-branch-coverage.py "$REPORT"
coverage_status=$?
set -e

# O HTML ajuda a investigar os ramos que não passaram pelo gate.
cargo +nightly llvm-cov report --branch --html --output-dir "$HTML_DIR"
printf '\nRelatório JSON: %s\nRelatório HTML: %s/index.html\n' "$REPORT" "$HTML_DIR"
if [[ $coverage_status -ne 0 ]]; then
  echo "FALHA: o Functional Core não alcançou 100% de cobertura de regiões e ramos. Consulte o relatório, adicione testes ou simplifique ramos inalcançáveis." >&2
  exit "$coverage_status"
fi
echo "OK: 100% de regiões de produção e resultados de ramos do escopo do Functional Core."
