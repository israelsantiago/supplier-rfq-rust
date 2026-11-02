#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
ACTION="${1:-}"
GATE="${2:-}"
case "$ACTION:$GATE" in
  invalidate:context-verify|record:context-verify|verify:context-verify|\
  invalidate:context-coverage|record:context-coverage|verify:context-coverage|\
  invalidate:context-mutation|record:context-mutation|verify:context-mutation|\
  invalidate:factstr-integration|record:factstr-integration|verify:factstr-integration) ;;
  *) echo "Uso: bash scripts/amri-stage3-gate-state.sh {invalidate|record|verify} {context-verify|context-coverage|context-mutation|factstr-integration}" >&2; exit 2 ;;
esac

CONTEXT_FILES=(
  src/application_state.rs
  src/application_state/atomic_interaction/context.rs
  src/application_state/atomic_interaction/state_port.rs
  src/application_state/atomic_interaction/fake.rs
  scripts/amri-stage3-gate-state.sh
)
case "$GATE" in
  context-verify)
    SOURCE_FILES=("${CONTEXT_FILES[@]}" scripts/verify-amri-stage3-context.sh)
    ;;
  context-coverage)
    SOURCE_FILES=("${CONTEXT_FILES[@]}" scripts/check-amri-stage3-context-coverage.py scripts/coverage-amri-stage3-context.sh)
    ;;
  context-mutation)
    SOURCE_FILES=("${CONTEXT_FILES[@]}" scripts/mutate-amri-stage3-context.sh)
    ;;
  factstr-integration)
    SOURCE_FILES=("${CONTEXT_FILES[@]}" src/application_state/atomic_interaction/mod.rs src/application_state/atomic_interaction/factstr.rs tests/amri_factstr_state.rs scripts/verify-amri-stage3-factstr.sh)
    ;;
esac
for path in "${SOURCE_FILES[@]}"; do
  [[ -f "$path" ]] || { echo "ERRO: fonte de gate ausente: $path" >&2; exit 2; }
done

STATE_DIR="$ROOT/target/amri-stage3"
mkdir -p "$STATE_DIR"
MARKER="$STATE_DIR/$GATE.ok"
MANIFEST="$STATE_DIR/$GATE.source.sha256"
case "$ACTION" in
  invalidate)
    rm -f -- "$MARKER" "$MANIFEST"
    echo "INVALIDATED: $GATE"
    ;;
  record)
    tmp="$MANIFEST.tmp.$$"
    trap 'rm -f -- "$tmp"' EXIT
    sha256sum -- "${SOURCE_FILES[@]}" > "$tmp"
    mv -f -- "$tmp" "$MANIFEST"
    : > "$MARKER"
    echo "RECORDED: $GATE source fingerprint"
    ;;
  verify)
    [[ -f "$MARKER" && -f "$MANIFEST" ]] || { echo "FALHA: gate $GATE sem marcador/fingerprint." >&2; exit 1; }
    tmp="$STATE_DIR/$GATE.verify.tmp.$$"
    trap 'rm -f -- "$tmp"' EXIT
    sha256sum -- "${SOURCE_FILES[@]}" > "$tmp"
    if ! cmp -s -- "$MANIFEST" "$tmp"; then
      echo "FALHA: fontes do Stage 3 mudaram depois do gate $GATE; reexecute-o." >&2
      diff -u -- "$MANIFEST" "$tmp" >&2 || true
      exit 1
    fi
    echo "PASS: fingerprint do gate $GATE corresponde às fontes atuais."
    ;;
esac
