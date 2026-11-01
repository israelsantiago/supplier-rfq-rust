#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
BRANCH="feat/amri-stage2-core"

[[ "$(git branch --show-current)" == "$BRANCH" ]] || {
  echo "ERRO: finalize deve rodar na branch $BRANCH." >&2
  exit 2
}
for gate in core-verify coverage mutation; do
  bash scripts/amri-stage2-gate-state.sh verify "$gate"
done

set +e
REMOTE_REF_OUTPUT="$(git ls-remote --exit-code --heads origin "refs/heads/$BRANCH" 2>&1)"
REMOTE_REF_STATUS=$?
set -e
if [[ $REMOTE_REF_STATUS -eq 0 && -n "$REMOTE_REF_OUTPUT" ]]; then
  echo "ERRO: a branch remota $BRANCH já existe; não será sobrescrita." >&2
  exit 2
elif [[ $REMOTE_REF_STATUS -ne 2 ]]; then
  echo "ERRO: não foi possível confirmar a ausência da branch remota. Nada será publicado." >&2
  printf '%s\n' "$REMOTE_REF_OUTPUT" >&2
  exit 2
fi

printf '\n== 1. Regressão completa do repositório ==\n'
bash scripts/verify-rpu-boundaries.sh
bash scripts/verify-openapi.sh
sh scripts/verify-mermaid-diagrams.sh
bash scripts/verify-documentation-current-state.sh
bash scripts/verify-amri-core-boundaries.sh

printf '\n== 2. Marcar Stage 2 e atualizar matriz/ADR com evidência real ==\n'
python3 scripts/update-amri-stage2-docs.py

printf '\n== 3. Validar documentação após a atualização ==\n'
bash scripts/verify-documentation-current-state.sh
sh scripts/verify-mermaid-diagrams.sh

git diff --check

EXPECTED_PATHS=(
  .openidea/.gitignore
  .github/workflows/ci.yml
  src/lib.rs
  src/amri/mod.rs
  src/amri/identity.rs
  src/amri/definition.rs
  src/amri/execution.rs
  src/amri/outcome.rs
  src/amri/plan.rs
  src/amri/error.rs
  src/amri/tests.rs
  scripts/verify-amri-core-boundaries.py
  scripts/verify-amri-core-boundaries.sh
  scripts/verify-amri-core.sh
  scripts/check-amri-core-branch-coverage.py
  scripts/coverage-amri-core.sh
  scripts/mutate-amri-core.sh
  scripts/amri-stage2-gate-state.sh
  scripts/update-amri-stage2-docs.py
  scripts/finalize-amri-stage2.sh
  docs/architecture/amri/implementation-plan.md
  docs/architecture/adr/0034-atomic-multi-rpu-interaction.md
  docs/testing/architecture-evidence.md
)

printf '\n== 4. Verificar allowlist antes do stage ==\n'
assert_allowed_path() {
  local path="$1"
  local expected
  for expected in "${EXPECTED_PATHS[@]}"; do
    [[ "$path" == "$expected" ]] && return 0
  done
  echo "ERRO: arquivo fora da allowlist da Etapa 2: $path" >&2
  exit 2
}

for path in "${EXPECTED_PATHS[@]}"; do
  [[ -f "$path" ]] || {
    echo "ERRO: arquivo esperado ausente ou não regular: $path" >&2
    exit 2
  }
done

while IFS= read -r -d '' path; do
  assert_allowed_path "$path"
done < <(git diff --cached --name-only -z HEAD)
while IFS= read -r -d '' path; do
  assert_allowed_path "$path"
done < <(git diff --name-only -z)
while IFS= read -r -d '' path; do
  assert_allowed_path "$path"
done < <(git ls-files --others --exclude-standard -z)

printf '\n== 5. Stage explícito dos arquivos da Etapa 2 ==\n'
git add -- "${EXPECTED_PATHS[@]}"
git diff --cached --check

echo "--- git status --short ---"
git status --short
echo "--- git diff --cached --stat ---"
git diff --cached --stat
echo "--- git diff --cached --name-status ---"
git diff --cached --name-status

printf '\n========== DIFF STAGED COMPLETO ==========\n'
git --no-pager diff --cached --no-ext-diff --
printf '\nPara criar o commit e fazer push sem force, digite PUBLICAR: '
read -r confirmation
if [[ "$confirmation" != "PUBLICAR" ]]; then
  echo "Operação interrompida pelo usuário. Arquivos continuam staged; nenhum commit/push foi feito."
  exit 0
fi

COMMIT_MESSAGE="feat(amri): implement minimal pure core primitives"
git commit -m "$COMMIT_MESSAGE"
LOCAL_SHA="$(git rev-parse HEAD)"

echo "== Push sem force =="
if ! git push --set-upstream origin "$BRANCH"; then
  echo "ERRO: o commit local já foi criado, mas o push falhou." >&2
  echo "Não recrie nem altere o commit. Após corrigir conectividade/permissão, execute:" >&2
  printf '  git -C %q push --set-upstream origin %q\n' "$ROOT" "$BRANCH" >&2
  printf 'Commit local preservado: %s\n' "$(git rev-parse HEAD)" >&2
  exit 3
fi
set +e
REMOTE_LOOKUP="$(git ls-remote origin "refs/heads/$BRANCH" 2>&1)"
REMOTE_LOOKUP_STATUS=$?
set -e
if [[ $REMOTE_LOOKUP_STATUS -ne 0 ]]; then
  echo "ERRO: commit foi enviado, mas não foi possível consultar a referência remota para confirmar o SHA." >&2
  printf '%s\n' "$REMOTE_LOOKUP" >&2
  echo "Commit local: $LOCAL_SHA" >&2
  echo "Após restabelecer a conectividade, confira com:" >&2
  printf '  git -C %q ls-remote origin %q\n' "$ROOT" "refs/heads/$BRANCH" >&2
  exit 3
fi
REMOTE_SHA="$(awk 'NR == 1 {print $1}' <<< "$REMOTE_LOOKUP")"
if [[ -z "$REMOTE_SHA" || "$REMOTE_SHA" != "$LOCAL_SHA" ]]; then
  echo "ERRO: SHA remoto não corresponde ao commit local." >&2
  echo "Local:  $LOCAL_SHA" >&2
  echo "Remote: ${REMOTE_SHA:-<ausente>}" >&2
  exit 3
fi

if [[ -n "$(git status --porcelain=v1 --untracked-files=all)" ]]; then
  echo "ERRO: worktree não ficou limpo após push." >&2
  git status --short >&2
  exit 3
fi

echo "PASS: Stage 2 commit e push verificados."
echo "Branch: $BRANCH"
echo "Commit: $LOCAL_SHA"
echo "Remote SHA: $REMOTE_SHA"
echo "Worktree: CLEAN"
