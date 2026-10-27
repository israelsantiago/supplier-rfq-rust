# ADR-0005: FACTSTR como Event Store autoritativo

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

O fluxo precisa manter fatos imutáveis, consultar contexto e rejeitar gravações baseadas em contexto obsoleto.

## Decisão

Usar FACTSTR 0.5.x como contrato de Event Store, com backend PostgreSQL. A API da aplicação utiliza `query`, `append` e `append_if`.

## Consequências

A aplicação não deve gravar diretamente nas tabelas internas do Event Store. Sem benchmarks adequados, a escolha não implica garantia de throughput de produção.
