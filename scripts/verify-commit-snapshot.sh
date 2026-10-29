#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
fail() { echo "FALHA: $*" >&2; exit 1; }

git diff --quiet -- || fail "há alterações unstaged; o snapshot testado não corresponde ao index"
UNTRACKED="$(git ls-files --others --exclude-standard)"
[[ -z "$UNTRACKED" ]] || { printf 'FALHA: arquivos untracked não staged:\n%s\n' "$UNTRACKED" >&2; exit 1; }

python3 - <<'PY_CHECK'
import subprocess
import sys
checks = {
    "src/projections/rfq_summary_projection/mod.rs": "impl RfqSummaryReader for RfqSummaryProjection",
    "scripts/verify-rpu-boundaries.sh": "command -v rg",
    ".github/workflows/ci.yml": "apt-get install --yes ripgrep",
    "scripts/verify-commit-snapshot.sh": "git diff --quiet",
}
for path, marker in checks.items():
    result = subprocess.run(["git", "show", f":{path}"], capture_output=True, text=True)
    if result.returncode != 0:
        print(f"FALHA: {path} não consta no index/staging", file=sys.stderr)
        sys.exit(1)
    if marker not in result.stdout:
        print(f"FALHA: snapshot staged de {path} sem marcador esperado: {marker}", file=sys.stderr)
        sys.exit(1)
print("OK: worktree/index alinhados; adaptador e correções fail-closed estão staged.")
PY_CHECK

git diff --cached --check
echo "OK: integridade do snapshot staged validada."
