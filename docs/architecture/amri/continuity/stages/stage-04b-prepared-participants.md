# Stage 4B — Prepared Participants RFQ

**Estado em 2026-11-03:** Implementada e publicada. Commit `5d278958855e41277bd00573d2cf419a50ee875d` (`feat(amri): add prepared RFQ sourcing participants (Stage 4B)`). A validação local deve ser registrada em `quality-and-evidence.md`; este checkpoint não afirma PASS de Cargo/Clippy.

## Objetivo

Demonstrar três adapters estáticos, tipados e sem I/O que chamam as decisões Stage 4A e produzem `PreparedOutcome`/`ProposedFact` do AMRI Core. Esta implementação está publicada no commit indicado no cabeçalho.

## Arquivos de implementação publicados

- `src/lib.rs`: registra o módulo experimental `amri_participants`; `crate::domain` permanece `pub(crate)`.
- `src/amri_participants/mod.rs`: módulo de aplicação com reexports seletivos.
- `src/amri_participants/rfq_sourcing.rs`: participantes, contratos de saída, metadata declarativa de `InteractionDefinition` e testes.
- `docs/architecture/amri/stage4b-prepared-participants.md`: nota de design da subetapa.

## Participantes

| Participante | Entrada | Saída | Decisão compartilhada |
|---|---|---|---|
| `CloseRfqParticipant` | `CloseRfqRequest`, `RfqLifecycleContext`, timestamp e `FactId` | `RfqClosedFact` + `RfqSourcingFact::RfqClosed` | `decide_close_rfq` |
| `AwardSupplierQuoteParticipant` | `AwardQuoteSelection`, `SourcingContext`, timestamp/award ID/FactId | `AwardedSupplierQuote` + `RfqSourcingFact::RfqAwarded` | `decide_award_supplier_quote` |
| `CreatePurchaseOrderParticipant` | `AwardedSupplierQuote`, PO ID, timestamp e `FactId` | `PurchaseOrderCreatedFact` + `RfqSourcingFact::PurchaseOrderCreated` | `decide_create_purchase_order` |

## Dependências

- `CloseRfqParticipant`: sem predecessor.
- `AwardSupplierQuoteParticipant`: sem predecessor; recebe projeção do mesmo snapshot pre-interação.
- `CreatePurchaseOrderParticipant`: depende de Award; consome o hand-off imutável.

Não adicionar dependência Close → Award. O fato de Close ser proposto não altera o contexto visto por Award antes de qualquer commit. O fechamento e award são consequências propostas em conjunto, e políticas de composição da Stage 6 validarão as relações entre fatos.

## Invariantes

- Nada de `ApplicationStateProvider`, `AtomicInteractionStatePort`, `AtomicSourcingStateProvider`, `ClockProvider`, `IdProvider`, `append_if`, SQLx, Tokio, Axum, FACTSTR ou `async` no participante.
- Nada de chamar `process_with_providers` ou shell de outra capability.
- Valores de timestamp e IDs são fornecidos pelo chamador e estáveis por execução lógica.
- `AwardedSupplierQuote` permanece o contrato para transferir award + quote original/linhas.
- O Core genérico `src/amri` permanece sem dependência de Supply Chain.
- A `InteractionDefinition` é somente metadata, não executor.

## Public visibility — decisão implementada

A implementação publicada expõe `crate::amri_participants` como módulo experimental e reexporta seletivamente os tipos de domínio necessários às assinaturas. `crate::domain` permanece `pub(crate)`; a árvore interna inteira não é promovida a API pública. Não contornar warnings com lint suppression nem ampliar a superfície pública sem uma decisão de API específica.

## Histórico — correção do patcher V1/V2 antes da publicação Stage 4B

Na preparação anterior à publicação, a primeira tentativa de patcher teve bloqueio de formatação/resolução de módulos; a revisão subsequente passou a formatar o arquivo do participante isoladamente e reservar `cargo fmt --all -- --check` para a árvore real. Este registro é histórico, não uma instrução operacional vigente. O código Stage 4B foi publicado em `5d278958855e41277bd00573d2cf419a50ee875d`.

## Testes puros mínimos

1. Close sucesso e rejeição, fato e timestamp.
2. Award sucesso e rejeição, award ID/instante e hand-off com cotação fonte idêntica.
3. PO consome apenas `AwardedSupplierQuote`; mantém supplier, quote ID, totais, termos e itens.
4. Graph declara três participantes; Close e Award independentes, PO depende de Award.
5. Nenhuma dependência de state/write/Provider no código do participante.
6. IDs de participantes e fact types estáveis e unívocos.
7. Confirmar que nenhum caminho de rejeição retorna facts preparados.

## Evidências de verificação ainda pendentes

Os gates abaixo são a referência para confirmar a implementação no checkout local. Não registrar PASS sem logs reais e contagem/exit code observados: `cargo fmt --all -- --check`, `cargo check --all-targets --all-features --locked`, testes de library com filtros puros, `cargo clippy --all-targets --all-features --locked -- -D warnings`, boundary guards e `git diff --check`. Não executar Testcontainers, PostgreSQL ou mutation testing como parte da subetapa pura.

O commit Stage 4B já está publicado. Após recuperar/executar os gates locais, registrar os resultados em `quality-and-evidence.md`; não repetir aplicação, commit ou push da implementação Stage 4B. A próxima fase arquitetural é Stage 5, conforme `stage-05-reactor-and-providers.md`.## Explicitamente fora de Stage 4B

Reactor, execução topológica, Provider, composite context acquisition, composição de domínio, commit plan, append, interaction marker, retries, idempotência e recovery. Esses elementos continuam nas Stages 5, 6 e 7.
