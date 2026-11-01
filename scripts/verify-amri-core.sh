#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

bash scripts/amri-stage2-gate-state.sh invalidate core-verify

printf '\n== 1. Boundaries: Core puro e sem infraestrutura ==\n'
bash scripts/verify-amri-core-boundaries.sh

printf '\n== 2. Formatação Rust ==\n'
cargo fmt --all -- --check

printf '\n== 3. Check da library sem features ==\n'
cargo check --lib --no-default-features --locked

printf '\n== 4. Testes unitários AMRI (sem integration tests/Testcontainers) ==\n'
DOCKER_HOST="unix:///tmp/amri-core-must-not-use-docker.sock" \
  cargo test --lib --no-default-features --locked amri:: -- --test-threads=1

printf '\n== 5. Clippy da library sem features ==\n'
cargo clippy --lib --no-default-features --locked -- -D warnings

printf '\nPASS: AMRI Core boundaries, fmt, check, testes puros e Clippy.\n'
bash scripts/amri-stage2-gate-state.sh record core-verify
