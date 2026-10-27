# ADR-0006: Command Context Consistency (CCC)

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

Uma decisão é válida apenas enquanto os fatos relevantes para essa decisão não mudaram.

## Decisão

Cada comando define uma `EventQuery` de contexto e chama `append_if` usando a `current_context_version` observada. A query de contexto faz parte da regra de negócio.

## Consequências

Queries muito amplas produzem conflitos falsos; queries estreitas podem permitir decisões inválidas. Toda nova regra exige rever a query e testar contexto obsoleto.
