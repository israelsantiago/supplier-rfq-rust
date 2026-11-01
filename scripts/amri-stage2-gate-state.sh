#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

ACTION="${1:-}"
GATE="${2:-}"
case "$ACTION:$GATE" in
  invalidate:core-verify|record:core-verify|verify:core-verify|invalidate:coverage|record:coverage|verify:coverage|invalidate:mutation|record:mutation|verify:mutation) ;;
  *)
    echo "Uso: bash scripts/amri-stage2-gate-state.sh {invalidate|record|verify} {core-verify|coverage|mutation}" >&2
    exit 2
    ;;
esac

STATE_DIR="$ROOT/target/amri-stage2"
mkdir -p "$STATE_DIR"
MARKER="$STATE_DIR/$GATE.ok"
MANIFEST="$STATE_DIR/$GATE.source.sha256"

SOURCE_FILES=(
  Cargo.toml
  Cargo.lock
  src/lib.rs
  src/amri/mod.rs
  src/amri/identity.rs
  src/amri/definition.rs
  src/amri/execution.rs
  src/amri/outcome.rs
  src/amri/plan.rs
  src/amri/error.rs
  src/amri/tests.rs
  .github/workflows/ci.yml
  scripts/verify-amri-core-boundaries.py
  scripts/verify-amri-core-boundaries.sh
  scripts/verify-amri-core.sh
  scripts/check-amri-core-branch-coverage.py
  scripts/coverage-amri-core.sh
  scripts/mutate-amri-core.sh
  scripts/amri-stage2-gate-state.sh
  scripts/update-amri-stage2-docs.py
  scripts/finalize-amri-stage2.sh
)
for path in "${SOURCE_FILES[@]}"; do
  [[ -f "$path" ]] || { echo "ERRO: fonte de gate ausente: $path" >&2; exit 2; }
done

case "$ACTION" in
  invalidate)
    rm -f -- "$MARKER" "$MANIFEST"
    echo "INVALIDATED: $GATE"
    ;;
  record)
    temp="$MANIFEST.tmp.$$"
    trap 'rm -f -- "$temp"' EXIT
    sha256sum -- "${SOURCE_FILES[@]}" > "$temp"
    mv -f -- "$temp" "$MANIFEST"
    : > "$MARKER"
    echo "RECORDED: $GATE source fingerprint"
    ;;
  verify)
    [[ -f "$MARKER" && -f "$MANIFEST" ]] || {
      echo "FALHA: gate $GATE não possui marcador e fingerprint." >&2
      exit 1
    }
    temp="$STATE_DIR/$GATE.verify.tmp.$$"
    trap 'rm -f -- "$temp"' EXIT
    sha256sum -- "${SOURCE_FILES[@]}" > "$temp"
    if ! cmp -s -- "$MANIFEST" "$temp"; then
      echo "FALHA: fontes AMRI/Cargo mudaram depois do gate $GATE; reexecute-o." >&2
      diff -u -- "$MANIFEST" "$temp" >&2 || true
      exit 1
    fi
    echo "PASS: fingerprint do gate $GATE corresponde às fontes atuais."
    ;;
esac
