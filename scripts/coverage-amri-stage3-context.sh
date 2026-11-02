#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
bash scripts/amri-stage3-gate-state.sh invalidate context-coverage
bash scripts/verify-amri-stage3-context.sh
command -v rustup >/dev/null 2>&1 || { echo "ERRO: rustup não encontrado." >&2; exit 2; }
if ! rustup run nightly rustc --version >/dev/null 2>&1; then
  rustup toolchain install nightly --component llvm-tools-preview
else
  rustup component add llvm-tools-preview --toolchain nightly
fi
if ! cargo +nightly llvm-cov --version >/dev/null 2>&1; then
  cargo install --locked cargo-llvm-cov
fi
REPORT="$ROOT/target/amri-stage3-context-coverage.json"
HTML_DIR="$ROOT/target/amri-stage3-context-coverage-html"
mkdir -p "$ROOT/target"
cargo +nightly llvm-cov clean --workspace
printf '\n== Coverage 100%% dos módulos puros Stage 3A; somente library tests ==\n'
DOCKER_HOST="unix:///tmp/amri-stage3-fake-must-not-use-docker.sock" \
  cargo +nightly llvm-cov --branch --lib --no-default-features --locked \
    --json --output-path "$REPORT" -- application_state::atomic_interaction:: -- --test-threads=1
python3 scripts/check-amri-stage3-context-coverage.py "$REPORT"
cargo +nightly llvm-cov report --branch --html --output-dir "$HTML_DIR"
printf 'JSON: %s\nHTML: %s/index.html\n' "$REPORT" "$HTML_DIR"
bash scripts/amri-stage3-gate-state.sh record context-coverage
