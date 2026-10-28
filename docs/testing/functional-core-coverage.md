# Functional Core — coverage gate

## Objetivo

O quality gate cobre **regiões de código de produção e resultados dos ramos** nos módulos puros listados em `scripts/check-functional-core-branch-coverage.py`. O código a partir de `#[cfg(test)]` fica fora do cálculo. O gate exige que cada região de código do core tenha contador de execução positivo e que os dois resultados de cada condição (true/false) tenham sido exercitados.

A meta de 100% é aplicada a este escopo explícito do Functional Core — não é uma afirmação de que todo o crate, todo o shell de I/O ou todos os possíveis estados de produção estão 100% testados. A cobertura mede execução, não suficiência das assertions; por isso ela é complementada por proptest, testes determinísticos de borda e, na Etapa 16, mutation testing.

## Local

O runner tenta preparar automaticamente Rust nightly, `llvm-tools-preview` e `cargo-llvm-cov` quando faltarem. Ele exige conexão de rede para baixar as ferramentas e Docker acessível porque os testes do repositório usam Testcontainers. Esses pré-requisitos são verificados antes de executar a medição.

```bash
bash scripts/coverage-functional-core.sh
```

A saída JSON e o HTML são criados em `target/functional-core-branch-coverage.json` e `target/functional-core-branch-coverage-html/index.html`. Se o gate falhar, adicione testes que exercitem os ramos legítimos. Se um ramo for inalcançável por uma invariante anterior, prefira simplificar/refatorar o core em vez de testar artificialmente um estado impossível.

## Ferramenta e limitação

`cargo-llvm-cov --branch` requer nightly neste fluxo porque o branch coverage é instável/opcional no cargo-llvm-cov. O script falha fechado se o JSON da versão instalada não tiver counters reconhecidos — não converte um relatório desconhecido em um falso sucesso.

## Fronteiras fora do Functional Core

A meta de 100% aplica-se apenas aos arquivos listados em `scripts/check-functional-core-branch-coverage.py`. Ela não mede `ApplicationStateProvider`, seu adaptador FACTSTR, as fachadas, o middleware HTTP, migrations ou estados operacionais. Esses componentes usam teste de contrato, integração PostgreSQL, concorrência e injeção de falha; veja [`application-state-provider.md`](application-state-provider.md) e [`http-idempotency-testing.md`](http-idempotency-testing.md).
