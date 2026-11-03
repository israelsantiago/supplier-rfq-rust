# Stage 3 — Composite Context e Atomic Interaction State Port

**Estado:** Implementada e verificada anteriormente; documentação registra a prova com FACTSTR 0.5.2 em 2026-11-01.\
**Código principal:** `src/application_state.rs` e submódulo `atomic_interaction`; detalhes em `stage-3-composite-context-state-port.md`.

## Objetivo

Provar uma única leitura coerente para a união de requisitos e um token opaco que protege todos os fatos relevantes para a interação.

## Conteúdo implementado conforme documentação existente

- `ContextRequirement` e `CompositeContextSpecification` com união/deduplicação determinística.
- Canonicalização JSON versionada e fingerprint da especificação.
- `SnapshotEnvelope` e token opaco vinculado ao adapter/store.
- `AtomicInteractionStatePort` e fake state adapter.
- Adapter FACTSTR que preserva query e `current_context_version` da leitura para a operação condicional.
- Testes de semântica EventQuery OR/AND, alteração relevante/irrelevante, stale token, batch all-or-none e corrida entre conexões independentes.

## Semântica de FACTSTR 0.5.2 documentada

Filtros EventQuery são alternativas OR; event type e um predicate de payload dentro de cada filtro formam AND; múltiplos payload predicates não são usados como conjunção de vários campos. A versão protegida é `current_context_version` do contexto completo, não o maior sequence number retornado.

O token é opaco ao Core. Adaptador associa token ao StoreIdentity e ao snapshot/specification; tokens não podem ser combinados ou trocados entre specs/stores.

## Fora de escopo

Sem Reactor, RFQ prepared participants, policies, interaction marker, idempotent recovery, HTTP endpoint e interação RFQ multi-RPU de ponta a ponta.

## Evidência histórica

O plano Stage 3 registra verificador puro, cobertura 100% scoped, mutation deferred e acceptance tests PostgreSQL/Testcontainers para o adapter. O relatório de teste completo deve ser consultado no branch/CI quando a interação final for construída.

## Critério de saída

Uma leitura lógica, um snapshot coerente, um token opaco, query/token exatos usados no append e testes de concorrência relevantes. Não extrapolar a prova para idempotência ou recuperação AMRI, ainda não implementadas.
