# ADR-0020: Semântica explícita das EventQueries CCC

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

O escopo de uma query altera as decisões de consistência e concorrência.

## Decisão

Tratar filtros alternativos como alternativas OR e predicados do mesmo filtro conforme a semântica de FACTSTR; validar os contextos de identidade natural e de cotação com testes específicos.

## Consequências

Não modificar uma query apenas como otimização sem rever invariantes. `current_context_version` não deve ser confundida com a sequência do último evento retornado.
