#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

# The targeted library tests may start Testcontainers; fail clearly before installing tools.
command -v docker >/dev/null 2>&1 || {
  echo "ERRO: Docker CLI não encontrado; instale Docker antes de executar mutation testing." >&2
  exit 2
}
docker info >/dev/null 2>&1 || {
  echo "ERRO: Docker não está acessível; inicie o daemon antes de executar mutation testing." >&2
  exit 2
}

if ! cargo mutants --version >/dev/null 2>&1; then
  echo "== cargo-mutants não está instalado; instalando agora =="
  cargo install --locked --force cargo-mutants || {
    echo "ERRO: falha ao instalar cargo-mutants. Verifique a conectividade e execute: cargo install --locked --force cargo-mutants" >&2
    exit 2
  }
fi
cargo mutants --version >/dev/null 2>&1 || {
  echo "ERRO: cargo-mutants continua indisponível após a instalação." >&2
  exit 2
}

JOBS="${MUTANTS_JOBS:-2}"
TIMEOUT="${MUTANTS_TIMEOUT:-90}"
[[ "$JOBS" =~ ^[1-9][0-9]*$ ]] || { echo "MUTANTS_JOBS deve ser inteiro positivo." >&2; exit 2; }
[[ "$TIMEOUT" =~ ^[1-9][0-9]*$ ]] || { echo "MUTANTS_TIMEOUT deve ser inteiro positivo." >&2; exit 2; }

echo "Mutation testing dirigido aos módulos puros do Functional Core."
echo "Jobs: $JOBS; timeout por mutant: ${TIMEOUT}s."
echo "A saída padrão do cargo-mutants será escrita em mutants.out/ (ignorada pelo Git)."

cargo mutants \
  --no-shuffle \
  --timeout "$TIMEOUT" \
  --jobs "$JOBS" \
  --file 'src/capabilities/*/generate_consequences.rs' \
  --file 'src/capabilities/*/build_context.rs' \
  --file 'src/capabilities/submit_supplier_quote/decision.rs' \
  --cargo-test-arg=--lib

cat <<'MSG'
Mutation run terminou. Revise os resultados de NOT CAUGHT/MISSED e TIMEOUT em mutants.out/.
Uma mutação não capturada não é automaticamente um defeito: avalie se ela representa um
comportamento relevante e acrescente assertion/teste quando necessário. Mutantes unviable
não contam como prova de cobertura. Não use o percentual de mutants como substituto de
branch coverage ou dos testes de CCC/aceitação.
MSG
