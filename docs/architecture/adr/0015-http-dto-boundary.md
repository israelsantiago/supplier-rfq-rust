# ADR-0015: DTOs HTTP separados dos fatos internos

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

Payloads persistidos não devem tornar-se automaticamente contratos públicos da API.

## Decisão

Converter DTOs de entrada para requests de RPU e fatos/resultados para DTOs de resposta explícitos.

## Consequências

Mudanças no esquema interno de fatos não devem alterar acidentalmente os contratos externos. Códigos HTTP e erros fazem parte do contrato.
