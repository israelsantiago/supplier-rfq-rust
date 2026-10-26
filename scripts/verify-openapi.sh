#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

[[ -f docs/openapi.yaml ]] || {
  echo "ERRO: docs/openapi.yaml não encontrado." >&2
  exit 2
}

if command -v node >/dev/null 2>&1 && command -v npm >/dev/null 2>&1; then
  NODE_MAJOR="$(node -p 'Number(process.versions.node.split(".")[0])')"
  if [[ "$NODE_MAJOR" -ge 22 ]]; then
    npx --yes @redocly/cli@1 lint --extends minimal docs/openapi.yaml
    exit $?
  fi
  echo "Node.js local é anterior à versão 22; usando contêiner node:22 para evitar instalar ferramentas no host."
fi

command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1 || {
  echo "ERRO: instale Node.js 22+ com npm, ou disponibilize Docker para executar node:22." >&2
  exit 2
}
docker run --rm \
  --volume "$ROOT:/workspace" \
  --workdir /workspace \
  node:22-bookworm-slim \
  sh -lc 'npx --yes @redocly/cli@1 lint --extends minimal docs/openapi.yaml'
