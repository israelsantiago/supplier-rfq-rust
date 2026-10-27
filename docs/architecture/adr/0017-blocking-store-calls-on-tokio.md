# ADR-0017: Isolar operações síncronas do FACTSTR em threads bloqueantes

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

A API HTTP usa Tokio/Axum, enquanto a interface `EventStore` do FACTSTR é síncrona.

## Decisão

Executar chamadas síncronas ao Event Store por `tokio::task::spawn_blocking` nos handlers e na inicialização de streams síncronos.

## Consequências

Evita bloquear diretamente threads de execução assíncrona. O comportamento de saturação do pool de blocking tasks deve ser observado sob carga.
