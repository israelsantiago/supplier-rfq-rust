# ADR-0016: Testes unitários, propriedades e aceitação com PostgreSQL

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

A correção funcional e as invariantes CCC precisam de evidências em diferentes níveis.

## Decisão

Usar testes unitários no Functional Core, proptest para propriedades matemáticas e Testcontainers/PostgreSQL para aceitação, replay e concorrência CCC.

## Consequências

Testes verdes demonstram somente cenários cobertos. Docker é necessário para os testes de aceitação baseados em containers; benchmarks e testes de falha operacional continuam sendo trabalhos distintos.

## Atualização — ASP e idempotência com falhas

A suíte deve testar cada camada com a evidência apropriada: Functional Core puro, adaptador ASP contra PostgreSQL real, concorrência CCC entre stores/conexões e estados/falhas do ledger HTTP. A meta de 100% de branches continua restrita aos arquivos explicitamente enumerados no checker. Os testes novos incluem claim concorrente, `in_progress`/`unknown`, falha ao persistir resposta, ledger indisponível antes do comando e ausência de duplicação de fato após falha pós-append. Testcontainers exige Docker.
