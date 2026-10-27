# ADR-0016: Testes unitários, propriedades e aceitação com PostgreSQL

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

A correção funcional e as invariantes CCC precisam de evidências em diferentes níveis.

## Decisão

Usar testes unitários no Functional Core, proptest para propriedades matemáticas e Testcontainers/PostgreSQL para aceitação, replay e concorrência CCC.

## Consequências

Testes verdes demonstram somente cenários cobertos. Docker é necessário para os testes de aceitação baseados em containers; benchmarks e testes de falha operacional continuam sendo trabalhos distintos.
