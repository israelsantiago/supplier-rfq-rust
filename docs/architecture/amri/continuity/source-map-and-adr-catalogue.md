# Mapa de fontes de verdade, código e catálogo completo de ADRs

Este documento ajuda uma nova conversa a localizar os artefatos existentes sem recriá-los ou assumir que o pacote de continuidade substitui o projeto original. Os links relativos apontam para arquivos já versionados no mesmo repositório.

## 1. Precedência das fontes

1. Código e testes da branch/commit remoto confirmado na retomada.
2. ADR individual aceito para a decisão estabelecida; ADR-0034 continua `Proposed`.
3. Documentos originais em `docs/architecture/amri/` e `docs/testing/architecture-evidence.md`.
4. Plano original e este pacote de continuidade, que consolidam status e acordos de trabalho.
5. Chats e resumos conversacionais, que indicam intenção e contexto, mas não provam execução.

Se houver divergência, registre-a; não escolha silenciosamente a versão mais conveniente. Se um documento disser `Implemented`, procure o código. Se disser `Verified`, procure comando/log/CI e commit. `Reported by maintainer` não é evidência independente do assistente.

## 2. Catálogo completo de ADRs

O índice oficial é [`docs/architecture/adr/README.md`](../../adr/README.md). Ele contém 34 decisões individuais, seu status e resumo. Não copiar os ADRs para este diretório: manter uma única fonte normativa reduz divergência.

| ADR | Tema | Estado no catálogo |
|---|---|---|
| [ADR-0001 — Rust platform](../../adr/0001-rust-platform.md) | Rust 2024 e plataforma | Accepted |
| [ADR-0002 — Axum HTTP adapter](../../adr/0002-axum-http-adapter.md) | Adaptador HTTP Axum | Accepted |
| [ADR-0003 — Domain Capabilities / RPU](../../adr/0003-domain-capabilities-rpu.md) | Capabilities como unidades de processamento | Accepted |
| [ADR-0004 — Functional Core / Imperative Shell](../../adr/0004-functional-core-imperative-shell.md) | Separação de decisão pura e shell | Accepted |
| [ADR-0005 — FACTSTR Event Store](../../adr/0005-factstr-event-store.md) | Event Store autoritativo | Accepted |
| [ADR-0006 — CCC](../../adr/0006-command-context-consistency.md) | Consistência do contexto de comando | Accepted |
| [ADR-0007 — Versioned immutable facts](../../adr/0007-versioned-immutable-facts.md) | Fatos imutáveis e versionados | Accepted |
| [ADR-0008 — Money in minor units](../../adr/0008-money-integer-minor-units.md) | Valores monetários em unidades menores inteiras | Accepted |
| [ADR-0009 — Identity and client IDs](../../adr/0009-identity-and-client-supplied-ids.md) | Identidade e IDs fornecidos pelo cliente | Accepted |
| [ADR-0010 — Clock and ID Providers](../../adr/0010-clock-and-id-providers.md) | Providers explícitos de relógio e IDs | Accepted |
| [ADR-0011 — Stable metadata on CCC retry](../../adr/0011-stable-command-metadata-on-ccc-retry.md) | Metadados estáveis por execução lógica | Accepted |
| [ADR-0012 — PostgreSQL storage](../../adr/0012-postgresql-storage.md) | PostgreSQL | Accepted |
| [ADR-0013 — Durable stream projection](../../adr/0013-durable-stream-projection.md) | Projeção por stream durável | Accepted |
| [ADR-0014 — SQLx read model](../../adr/0014-sqlx-application-owned-read-model.md) | Read model de propriedade da aplicação | Accepted |
| [ADR-0015 — HTTP DTO boundary](../../adr/0015-http-dto-boundary.md) | DTOs HTTP separados | Accepted |
| [ADR-0016 — Test strategy](../../adr/0016-test-strategy.md) | Estratégia de testes | Accepted |
| [ADR-0017 — Blocking store calls on Tokio](../../adr/0017-blocking-store-calls-on-tokio.md) | Isolamento de chamadas síncronas | Accepted |
| [ADR-0018 — Bounded CCC retry](../../adr/0018-bounded-ccc-retry.md) | Retry CCC limitado | Accepted |
| [ADR-0019 — Eligibility assignment](../../adr/0019-eligibility-assignment-semantics.md) | Elegibilidade como atribuição | Accepted |
| [ADR-0020 — EventQuery semantics](../../adr/0020-event-query-semantics.md) | Semântica das EventQueries | Accepted |
| [ADR-0021 — API security](../../adr/0021-api-security-deferred.md) | Segurança da API | Deferred |
| [ADR-0022 — HTTP idempotency](../../adr/0022-http-idempotency-deferred.md) | Idempotência HTTP | Accepted |
| [ADR-0023 — No cross-RPU orchestrator](../../adr/0023-no-cross-rpu-orchestrator.md) | Sem orquestrador entre RPUs no escopo original | Accepted |
| [ADR-0024 — Test support feature](../../adr/0024-test-support-feature.md) | Feature de suporte a testes | Accepted |
| [ADR-0025 — RPU command facade](../../adr/0025-rpu-command-facade.md) | Fachada de comandos | Accepted |
| [ADR-0026 — Application State Provider](../../adr/0026-application-state-provider.md) | Porta de Application State | Accepted |
| [ADR-0027 — HTTP idempotency failure tests](../../adr/0027-http-idempotency-failure-testing.md) | Falhas e concorrência da idempotência | Accepted |
| [ADR-0028 — Error redaction and correlation](../../adr/0028-http-error-redaction-request-correlation.md) | Redação de erros e correlação | Accepted |
| [ADR-0029 — Mainline documentation and CI](../../adr/0029-mainline-documentation-and-ci.md) | Documentação e CI | Accepted |
| [ADR-0030 — RFQ summary reader](../../adr/0030-rfq-summary-reader-port.md) | Porta de leitura de resumo | Accepted |
| [ADR-0031 — Atomic sourcing finalization](../../adr/0031-atomic-rfq-sourcing-finalization.md) | Uma capability, quatro fatos, um append condicional | Accepted / implementado na main |
| [ADR-0032 — Quote validity date](../../adr/0032-quote-validity-date-semantics.md) | Validade inclusiva na data da decisão | Accepted |
| [ADR-0033 — Specialized Application State ports](../../adr/0033-application-state-provider-specialized-ports.md) | Portas especializadas por contrato | Accepted |
| [ADR-0034 — Atomic Multi-RPU Interaction](../../adr/0034-atomic-multi-rpu-interaction.md) | Proposta de interação multi-capability all-or-none | **Proposed** |

O catálogo do repositório é normativo para a classificação; esta tabela é um índice de navegação, não uma segunda ADR.

## 3. Documentos originais AMRI

| Arquivo | Conteúdo |
|---|---|
| [`docs/architecture/amri/README.md`](../README.md) | Índice e status da proposta AMRI |
| [`baseline.md`](../baseline.md) | Baseline, proteção de código existente e inventário |
| [`protocol.md`](../protocol.md) | Responsabilidades e contratos conceituais |
| [`consistency-model.md`](../consistency-model.md) | Snapshot, token opaco, CCC, idempotência, falhas e recuperação proposta |
| [`implementation-plan.md`](../implementation-plan.md) | Plano original de Stages 0–9 e evidências por stage |
| [`stage-3-composite-context-state-port.md`](../stage-3-composite-context-state-port.md) | Desenho/evidência Stage 3, planner, port e adapter FACTSTR |
| [`applicability.md`](../applicability.md) | Critérios de uso e limites de generalização |
| [`atomic-rfq-sourcing-amri.md`](../../../flows/atomic-rfq-sourcing-amri.md) | Fluxo RFQ alvo |
| [`architecture-evidence.md`](../../../testing/architecture-evidence.md) | Matriz de princípios, testes, comandos e limites de evidência |

## 4. Mapa do código por fase

### Stage 2 — AMRI Core puro

`src/amri/`:

- `mod.rs` — API do Core;
- `identity.rs` — IDs/fingerprint e identidade de interação;
- `definition.rs` — participantes e grafo/dependências;
- `execution.rs` — contexto lógico da execução;
- `outcome.rs` — fatos propostos e resultados preparados;
- `plan.rs` — validação estrutural e plano selado;
- `error.rs`, `tests.rs` — erros e evidência pura.

### Stage 3 — Contexto composto e State Port

- `src/application_state.rs` — fachada/contratos gerais de Application State e export do submódulo;
- `src/application_state/atomic_interaction/` — contexto canônico, snapshot/token opaco, port, fake sob `cfg(test)` e adapter FACTSTR;
- `tests/amri_factstr_state.rs` — prova de integração FACTSTR/PostgreSQL/Testcontainers;
- `scripts/verify-amri-stage3-context.sh` e `scripts/verify-amri-stage3-factstr.sh` — gates separados do fake/contexto e do adapter real.

### Stage 4A — Shared Functional Core

- `src/domain/mod.rs`;
- `src/domain/rfq_lifecycle.rs` — contexto e decisão pura de fechamento;
- `src/domain/sourcing.rs` — contexto, award, hand-off `AwardedSupplierQuote` e decisão de pedido;
- `src/capabilities/close_rfq/build_context.rs`;
- `src/capabilities/close_rfq/generate_consequences.rs`;
- `src/capabilities/finalize_rfq_sourcing/decision.rs`;
- `src/lib.rs` — registro do módulo `domain` (na revisão Stage 4A é `pub(crate)`).

A Stage 4A já foi publicada no commit SHA registrado em `current-state.md`. Não executar de novo o publisher que exige o baseline pré-Stage4A.

### Stage 4B — Prepared Participants (implementada e publicada)

A implementação está publicada no commit `5d278958855e41277bd00573d2cf419a50ee875d`: `src/amri_participants/mod.rs`, `src/amri_participants/rfq_sourcing.rs` e registro em `src/lib.rs`. O módulo `crate::amri_participants` reexporta seletivamente os tipos de domínio necessários; `crate::domain` permanece `pub(crate)`. Os três participantes preparam resultados sem I/O ou writes. `InteractionDefinition` é metadado, não executor. A validação local de Rust deve ser evidenciada separadamente; Stage 5 Reactor/Providers permanece pendente.

### Stages 5–9 — ainda não implementar

Reactor/Providers, políticas de composição, commit plan, marcador/recovery, hardening e segundo domínio têm artefatos de planejamento. Não devem ser confundidos com código já existente.

## 5. Scripts e workflows relevantes

| Caminho | Papel / cautela |
|---|---|
| `scripts/verify-amri-core-boundaries.sh` | Guard de dependências do AMRI Core |
| `scripts/verify-amri-stage3-context.sh` | Evidência de Stage 3A; script histórico pode conter precondição de branch da época |
| `scripts/verify-amri-stage3-factstr.sh` | Evidência da integração Stage 3B; conferir branch/gates antes de reaproveitar |
| `scripts/verify-rpu-boundaries.sh` | Guardas arquiteturais gerais e regressão Rust |
| `scripts/verify-application-state-boundary.sh` | Impede vazamento direto de FACTSTR às capabilities |
| `scripts/verify-functional-core-test-isolation.sh` | Inventário e isolamento Testcontainers/mutation |
| `scripts/verify-documentation-current-state.sh` | Índices/documentação de mainline |
| `scripts/verify-mermaid-diagrams.sh` | Validação de diagramas Mermaid |
| `.github/workflows/ci.yml` | Jobs CI, coverage e mutation agendada/manual |

Os nomes/caminhos são um mapa para consulta; não executar cegamente um script histórico que ainda tenha `branch` fixo para uma etapa anterior. Verificar sua precondição antes de usar.

## 6. Conversas anteriores relevantes

O contexto consolidado também usa resumos dos chats `Avaliação de Reatores e RPUs`, `Próximos passos da Etapa 1`, `Verificação de aderência arquitetural`, `Avaliação arquitetural pragmática`, `Avaliação arquitetural DCB` e `Resolver bug Rust Cargo test`. Este pacote não finge ser uma transcrição integral: decisões são consolidadas em ADR, documentos, commits, código e registros `Reported by maintainer`.
