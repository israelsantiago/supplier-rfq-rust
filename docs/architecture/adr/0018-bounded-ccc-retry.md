# ADR-0018: Retries CCC limitados e sem loops cegos

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

Conflitos concorrentes comuns podem ser reavaliados, mas repetição ilimitada pode causar latência sem limite.

## Decisão

Para comandos que possuem retry, permitir uma releitura/reavaliação limitada; retornar rejeição de negócio atualizada quando identificável ou um conflito recuperável.

## Consequências

O retry não pode ignorar `append_if`, reutilizar contexto obsoleto ou duplicar efeitos externos. Valores por execução lógica, quando definidos, são estáveis durante tentativas internas.
