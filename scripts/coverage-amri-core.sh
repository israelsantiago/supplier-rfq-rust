#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

bash scripts/amri-stage2-gate-state.sh invalidate coverage

bash scripts/verify-amri-core-boundaries.sh
command -v rustup >/dev/null 2>&1 || { echo "ERRO: rustup não encontrado." >&2; exit 2; }

if ! rustup run nightly rustc --version >/dev/null 2>&1; then
  rustup toolchain install nightly --component llvm-tools-preview || {
    echo "ERRO: não foi possível instalar nightly + llvm-tools-preview." >&2
    exit 2
  }
else
  rustup component add llvm-tools-preview --toolchain nightly || {
    echo "ERRO: não foi possível instalar llvm-tools-preview." >&2
    exit 2
  }
fi

if ! cargo +nightly llvm-cov --version >/dev/null 2>&1; then
  cargo install --locked cargo-llvm-cov || {
    echo "ERRO: instalação de cargo-llvm-cov falhou." >&2
    exit 2
  }
fi

REPORT="$ROOT/target/amri-core-branch-coverage.json"
HTML_DIR="$ROOT/target/amri-core-branch-coverage-html"
mkdir -p "$ROOT/target"
cargo +nightly llvm-cov clean --workspace

printf '\n== Cobertura de regiões e branches AMRI Core; somente testes amri:: ==\n'
DOCKER_HOST="unix:///tmp/amri-core-must-not-use-docker.sock" \
  cargo +nightly llvm-cov --branch --lib --no-default-features --locked \
    --json --output-path "$REPORT" -- amri:: -- --test-threads=1

python3 scripts/check-amri-core-branch-coverage.py "$REPORT"
cargo +nightly llvm-cov report --branch --html --output-dir "$HTML_DIR"
printf 'JSON: %s\nHTML: %s/index.html\n' "$REPORT" "$HTML_DIR"
mkdir -p "$ROOT/target/amri-stage2"
bash scripts/amri-stage2-gate-state.sh record coverage
