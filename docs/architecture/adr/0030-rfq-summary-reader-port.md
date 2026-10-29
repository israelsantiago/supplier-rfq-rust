# ADR-0030: Porta de leitura para resumos de RFQ

- **Status:** Accepted
- **Data:** 2026-10-29
- **Escopo:** Prioridade 3; não altera o modelo de domínio nem o contrato HTTP.

## Contexto

As RPUs `GetRfqSummary` e `ListRfqSummaries` recebiam diretamente `RfqSummaryProjection`, acoplando o processamento da query ao adaptador PostgreSQL/SQLx. Isso dificultava testar a RPU sem banco e substituir/variar a implementação de leitura.

## Decisão

- Introduzir `RfqSummaryReader`, uma porta de leitura sem tipos SQLx na assinatura e independente do módulo de persistência.
- Manter `RfqSummary` e `RfqSummaryPage` como modelos de consulta compartilhados pela porta.
- Implementar a porta no adaptador `RfqSummaryProjection`; a linha SQL interna usa um tipo `RfqSummaryRow` com `FromRow` e é convertida explicitamente para o modelo da porta.
- Fazer `QueryRpus` e as duas RPUs de resumo dependerem da interface.
- Manter o Application State Provider autoritativo separado da projeção derivada; esta mudança não transforma o read model em fonte de verdade.
- Usar leitores falsos em testes unitários para validar resultados, mapeamento de erros e preservação dos parâmetros normalizados.

## Compatibilidade e testes

A forma dos DTOs de resumo, os status/códigos HTTP, a validação de `status`, `limit` e `offset`, a ordenação, a consistência eventual e as consultas SQL não mudam. Os testes de integração atuais continuam validando o adaptador PostgreSQL; novos testes exercitam as RPUs sem banco.

Critérios: `cargo fmt`, `cargo check --all-targets --all-features --locked`, testes, Clippy e guards arquiteturais verdes.

## Consequências e limites

O contrato é assíncrono por futures boxed para continuar utilizável por `Arc<dyn RfqSummaryReader>` sem nova dependência procedural. O adaptador ainda traduz erros de persistência para um erro de leitura da aplicação; métricas e política de retries continuam fora do escopo desta porta.

## Referências

- [Rico Fritzsche — Request Processing Units](https://architecture.ricofritzsche.me/concepts/request-processing-units/)
- [Rico Fritzsche — Functional Core / Imperative Shell](https://architecture.ricofritzsche.me/concepts/functional-core-imperative-shell/)
