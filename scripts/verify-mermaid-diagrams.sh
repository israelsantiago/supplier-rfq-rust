#!/bin/sh
set -eu
ROOT=$(git rev-parse --show-toplevel 2>/dev/null) || {
  echo 'ERRO: execute dentro do checkout Git.' >&2
  exit 2
}
cd "$ROOT"
exec python3 scripts/verify-mermaid-diagrams.py
