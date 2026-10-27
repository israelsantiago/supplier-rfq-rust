# ADR-0012: PostgreSQL como persistência

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

A solução requer Event Store persistente e uma projeção relacional consultável.

## Decisão

Usar PostgreSQL tanto no backend FACTSTR quanto na projeção da aplicação, com conexões configuradas por ambiente.

## Consequências

Operação, backup, restore, migrações, permissões e capacidade precisam ser validados para o ambiente real. O teste Testcontainers não substitui recuperação de produção.
