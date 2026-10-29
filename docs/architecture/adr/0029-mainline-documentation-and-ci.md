# ADR-0029: Documentação alinhada à main e verificação contínua

- **Status:** Accepted
- **Data:** 2026-10-29
- **Escopo:** Prioridade 2 de hardening.

## Contexto

README e guia de design continham afirmações de que a implementação FACTSTR ficava em `event-sourcing-factstr`, enquanto a branch remota ativa é `main`. O README também descrevia o Application State Provider como aguardando merge e negava a existência do middleware de idempotência HTTP.

## Decisão

- `main` é a branch de trabalho ativa e os filtros de push/pull request da CI monitoram somente `main`.
- A documentação apresenta a variante relacional como referência histórica, sem instruir o usuário a alternar para uma branch remota que não existe.
- O README distingue funcionalidades implementadas de garantias ainda limitadas e pendências de produção.
- `scripts/verify-documentation-current-state.sh` é executado no CI e falha quando regressões conhecidas de documentação/branch reaparecem.

## Testes e critérios de aceite

A verificação documental valida as frases e links de estado do README, a indexação dos ADRs e os filtros da CI. Os testes de Rust/OpenAPI/Mermaid permanecem em seus gates existentes.

## Consequências e limites

O guardrail verifica afirmações conhecidas, não prova que toda frase futura seja verdadeira. Mudanças estruturais relevantes continuam exigindo revisão humana e atualização das evidências.

## Referências

- [Repository README](../../../README.md)
- [Architecture Knowledge Base de Rico Fritzsche](https://architecture.ricofritzsche.me/)
