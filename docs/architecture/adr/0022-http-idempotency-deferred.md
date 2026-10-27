# ADR-0022: Idempotência de comandos HTTP

- **Status:** Deferred
- **Branch de referência:** `main`

## Contexto

Uma resposta HTTP pode se perder depois do commit, fazendo o cliente repetir uma operação já confirmada.

## Decisão

A implementação de referência ainda não define idempotência HTTP por chave. Antes de produção, decidir a chave, o escopo, o armazenamento e a recuperação da resposta original.

## Consequências

Idempotência HTTP é distinta de CCC e de ID estável durante retry interno; uma não substitui a outra.
