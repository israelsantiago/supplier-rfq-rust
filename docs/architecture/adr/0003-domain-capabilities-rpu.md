# ADR-0003: Domain Capabilities como unidades de processamento

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

Operações de negócio devem ser organizadas por comportamento e intenção, não por CRUD ou entidade técnica.

## Decisão

Cada capability/RPU implementa uma ação ou pergunta de domínio, por exemplo `SubmitSupplierQuote`, com seu próprio contexto, decisão e consequência.

## Consequências

A lógica de uma RPU não deve delegar decisões de domínio a outra RPU. Novas capacidades exigem definição própria de entrada, saída e invariantes.
