# ADR-0002: Axum como adaptador HTTP

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

A aplicação precisa expor uma API HTTP assíncrona e integrar-se ao ecossistema Tokio.

## Decisão

Usar Axum para roteamento, extração de parâmetros e montagem de respostas HTTP; manter os handlers como adaptadores de transporte.

## Consequências

As RPUs não devem depender de tipos de request/response do Axum. Testes de aceitação verificam o contrato HTTP.
