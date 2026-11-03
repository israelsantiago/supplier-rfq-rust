# Stage 2 — AMRI Core puro

**Estado:** Implementada e verificada anteriormente (2026-10-30, conforme plano existente).\
**Código:** `src/amri/`.

## Objetivo

Implementar o conjunto mínimo de primitivas reutilizáveis sem dependência de Supply Chain, FACTSTR, PostgreSQL, SQLx, Axum, application layer ou runtime assíncrono.

## Conteúdo implementado

- Identidades tipadas: Interaction/Definition/Participant/Fact/FactType/Correlation IDs.
- Fingerprint opaco de 256 bits; encoding/hash canônico é responsabilidade da aplicação.
- `InteractionIdentity` com proteção contra reuso de ID para request/definição diferentes.
- `ExecutionContext` com metadata capturada fora do Core.
- `ParticipantDefinition` e `InteractionDefinition`, versão não zero, validações de duplicidade/dependência, ciclos e ordem topológica estável.
- `PreparedOutcome` e `ProposedFact` tipados.
- `ValidatedCommitPlan`, validação de set completo, propriedade/tipo/IDs de fatos e ordenação determinística.
- Erros estruturais explícitos; nenhum `serde_json::Value` universal.

## Fora de escopo mantido

Sem Context Planner, snapshot/token, state port, FACTSTR adapter, Reactor, participant RFQ, Provider, persistence, HTTP, Testcontainers ou commit.

## Evidência histórica

O `implementation-plan.md` registra `verify-amri-core-boundaries.sh`, unit/property tests com filtro `amri::`, checks de library, coverage scoped e mutation scoped sem Docker, além dos gates de regressão existentes. Uma nova sessão deve consultar CI/branch antes de atribuir validade atual a um job antigo.

## Critério de saída

Core independente com testes puros; coverage/mutation somente na superfície declarada; gates existentes preservados. A existência do Core não significa AMRI operacional ou atomicidade comprovada.
