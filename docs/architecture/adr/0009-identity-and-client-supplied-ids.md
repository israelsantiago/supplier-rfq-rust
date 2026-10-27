# ADR-0009: Identidade por UUID e IDs explícitos opcionais

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

As entidades de RFQ, fornecedor e cotação precisam de identificadores estáveis.

## Decisão

Usar UUIDs. Para abertura de RFQ e cadastro de fornecedor, conservar a opção existente de receber ID explícito; gerar UUID v4 somente quando ausente.

## Consequências

O provider de IDs não deve ser chamado quando o cliente forneceu um ID. A autorização para permitir IDs controlados por clientes é uma decisão de contrato/API separada.
