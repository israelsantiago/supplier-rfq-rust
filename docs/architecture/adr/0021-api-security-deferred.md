# ADR-0021: Autenticação e autorização fora do escopo da referência

- **Status:** Deferred
- **Branch de referência:** `main`

## Contexto

O projeto é uma referência de arquitetura e não demonstra autenticação/autorização completas.

## Decisão

Não declarar a API pronta para exposição pública. Autenticação, autorização por RFQ/fornecedor, gestão de segredos e controles de abuso precisam de ADRs e implementação antes da produção.

## Consequências

Esta decisão é uma limitação explícita, não aprovação para operar sem segurança em produção.
