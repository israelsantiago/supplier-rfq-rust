# Stage 4B — Prepared Participants RFQ

**Estado em 2026-11-03:** Pendente; dry-run da implementação V1 abortou antes de alterar o repositório.\
**Base publicada conhecida:** commit Stage 4A `203589e4d2c23338c8040e7802fabc77203fd6a9` (a base deve ser atualizada após o checkpoint documental).

## Objetivo

Implementar três adapters estáticos, tipados e sem I/O que chamem as decisões Stage 4A e produzam `PreparedOutcome`/`ProposedFact` do AMRI Core.

## Arquivos de código propostos

- `src/lib.rs`: registrar `pub mod amri_participants;` e ampliar `domain` para público, se mantido o desenho de API pública experimental.
- `src/amri_participants/mod.rs`: exportar `rfq_sourcing`.
- `src/amri_participants/rfq_sourcing.rs`: três participantes, contratos de saída, metadata de InteractionDefinition e testes puros.
- `docs/architecture/amri/stage4b-prepared-participants.md`: design note da subetapa.

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

## Public visibility — revisão obrigatória

A V1 escolheu exportar `pub mod amri_participants` e promover `pub(crate) mod domain` para `pub mod domain`, porque os tipos de domínio aparecem nas assinaturas públicas. Isso evita `private_interfaces` e `dead_code` mas amplia a API crate. Manter como API experimental e revisar a necessidade de expor a árvore completa `domain::sourcing` no diff. Não contornar com lint suppression.

## Correção do patcher V1

O dry-run V1 falhou com diferenças de rustfmt e erros de resolução de módulos filhos em temporários. A V2 deve evitar opções instáveis do rustfmt. Ela formata e verifica apenas `rfq_sourcing.rs` como arquivo isolado (sem módulos filhos externos), preservando a configuração local do rustfmt quando existir. Os registros triviais de `src/lib.rs` e `amri_participants/mod.rs` são validados estruturalmente; após a aplicação, `cargo fmt --all -- --check` valida o módulo real completo. Nenhum arquivo do repo pode ser escrito se rustfmt falhar.

## Testes puros mínimos

1. Close sucesso e rejeição, fato e timestamp.
2. Award sucesso e rejeição, award ID/instante e hand-off com cotação fonte idêntica.
3. PO consome apenas `AwardedSupplierQuote`; mantém supplier, quote ID, totais, termos e itens.
4. Graph declara três participantes; Close e Award independentes, PO depende de Award.
5. Nenhuma dependência de state/write/Provider no código do participante.
6. IDs de participantes e fact types estáveis e unívocos.
7. Confirmar que nenhum caminho de rejeição retorna facts preparados.

## Gates de saída

`cargo fmt --all -- --check`, `cargo check --all-targets --all-features --locked`, testes de library com filtros puros, `cargo clippy --all-targets --all-features --locked -- -D warnings`, boundary guards e `git diff --check`. Nenhum Testcontainers/mutation/integration run nesta subfase.

Depois: revisar diff/allowlist, backup, commit separado e push fast-forward; consultar commit remoto, registrar evidência e só então iniciar Stage 5.

## Explicitamente fora de Stage 4B

Reactor, execução topológica, Provider, composite context acquisition, composição de domínio, commit plan, append, interaction marker, retries, idempotência e recovery. Esses elementos continuam nas Stages 5, 6 e 7.
