# ADR-0014: SQLx para persistência da projeção da aplicação

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

O read model da aplicação exige transações SQL, migrações e consultas relacionais tipadas.

## Decisão

Usar SQLx para a tabela de projeção, consultas, transações e migrações pertencentes à aplicação; não usar SQLx para escrever nas tabelas internas do FACTSTR.

## Consequências

A projeção deve ser reconstruível e idempotente. Migrações devem ser versionadas e testadas.
