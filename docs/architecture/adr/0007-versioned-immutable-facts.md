# ADR-0007: Fatos imutáveis e tipos versionados

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

Eventos persistidos precisam continuar interpretáveis durante a evolução do domínio.

## Decisão

Usar nomes versionados como `rfq_opened.v1`; representar correções e mudanças por novos fatos, sem atualizar fatos históricos.

## Consequências

Mudanças incompatíveis exigem nova versão e estratégia explícita de leitura/upcasting. O fato histórico não deve ser reescrito.
