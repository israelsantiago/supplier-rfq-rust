#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

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
docker info >/dev/null 2>&1 || { echo "ERRO: o Docker precisa estar ativo porque a suíte pode iniciar Testcontainers." >&2; exit 2; }

mkdir -p target
REPORT="target/functional-core-branch-coverage.json"
HTML_DIR="target/functional-core-branch-coverage-html"

echo "== Functional Core: instrumentação LLVM + branch coverage (nightly) =="
cargo +nightly llvm-cov clean --workspace
cargo +nightly llvm-cov --branch --lib --json --output-path "$REPORT"

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
