# Stage 4B — Prepared Supply Chain Participants

**Status:** implementação Stage 4B publicada no commit `5d278958855e41277bd00573d2cf419a50ee875d`. A ADR-0034 permanece **Proposed**; isto não declara AMRI concluído ou aceito. A publicação confirma presença do código, não substitui evidências locais de Cargo/Clippy/testes.

## Objetivo e fronteira pública

Adicionar adaptadores de aplicação opt-in, expostos como API pública experimental em `crate::amri_participants`, que transformam as decisões puras do Shared Functional Core (Stage 4A) em `PreparedOutcome` tipados do AMRI Core. A API reexporta seletivamente, em `crate::amri_participants::rfq_sourcing`, apenas os tipos de domínio necessários às assinaturas (`RfqLifecycleContext`, `RfqLifecycleRejection`, `AwardQuoteSelection`, `AwardedSupplierQuote`, `SourcingContext` e `SourcingRejection`). `crate::domain` permanece `pub(crate)`: não expomos a árvore completa de módulos, funções internas ou futuros tipos de domínio por acidente. Os participantes continuam experimentais e vinculados à ADR-0034 ainda Proposed; não se promete estabilidade semântica/semver. A preparação só retorna outputs e fatos propostos em memória. Nenhum participante lê estado, chama providers, persiste fatos ou executa efeitos externos.

## Participantes e contratos

| Participante | Entrada | Output tipado | Fato proposto |
|---|---|---|---|
| `CloseRfqParticipant` | `CloseRfqRequest`, `RfqLifecycleContext`, instante e `FactId` estáveis | `RfqClosedFact` | `RfqSourcingFact::RfqClosed` |
| `AwardSupplierQuoteParticipant` | `AwardQuoteSelection`, `SourcingContext`, instante, award ID e `FactId` estáveis | `AwardedSupplierQuote` selado | `RfqSourcingFact::RfqAwarded` |
| `CreatePurchaseOrderParticipant` | `AwardedSupplierQuote`, PO ID, instante e `FactId` estáveis | `PurchaseOrderCreatedFact` | `RfqSourcingFact::PurchaseOrderCreated` |

Os timestamps e identificadores são capturados e injetados pela camada chamadora. Os participantes não conhecem `ClockProvider`, `IdProvider`, `ApplicationStateProvider`, `AtomicInteractionStatePort`, FACTSTR, PostgreSQL, SQLx, Axum ou Reactor.

Os `FactTypeId` (`supply-chain.rfq.closed`, `supply-chain.rfq.awarded` e `supply-chain.purchase-order.created`) são identificadores semânticos do protocolo AMRI, não nomes de eventos serializados no FACTSTR. A Stage 7 deverá mapear cada identificador explicitamente para o event type versionado correspondente (`*.v1`) e provar o mapeamento por testes de contrato; não inferir nomes por convenção ou string manipulation.

## Dependências declaradas

- `CloseRfq` não depende de `AwardSupplierQuote`.
- `AwardSupplierQuote` não depende de `CloseRfq`.
- `CreatePurchaseOrder` depende de `AwardSupplierQuote`.

A independência entre Close e Award é intencional. Ambos devem receber projeções derivadas do mesmo snapshot coerente anterior à interação. A Stage 4B não adquire snapshot nem verifica identidade/token compartilhado; essa garantia pertence à integração da Stage 5 sobre a porta composta da Stage 3. A decisão de award valida `SourcingContext.closed == false`; transformar a proposta de fechamento em estado já fechado antes de avaliar award faria a adjudicação rejeitar sua própria interação. O fechamento e a adjudicação são consequências propostas no mesmo lote, não transições sequenciais visíveis entre participantes.

A dependência Award → Create PO é real: o pedido exige o hand-off validado e selado `AwardedSupplierQuote`, que carrega a adjudicação e a cotação fonte com suas linhas. `RfqAwardedFact` isoladamente não contém as linhas comerciais necessárias ao pedido.

## Limites desta subetapa

- O `InteractionDefinition` declara participantes, tipos de fatos autorizados e a dependência necessária; não executa o grafo.
- A leitura de um snapshot composto e a entrega das projeções tipadas permanecem na fronteira de Stage 3 / futura integração de Stage 5.
- Não há Reactor, execução topológica, Providers, políticas de composição, `ValidatedCommitPlan` de domínio, port de commit, `append_if`, marcador de interação, recuperação ou retry nesta subetapa.
- O AMRI Core genérico em `src/amri` permanece sem dependência de Supply Chain.
- Os participantes continuam fora de `src/amri`, em `src/amri_participants`.

## Evidências requeridas

1. Testes puros de sucesso e rejeição para Close e Award, preservando os erros do Functional Core.
2. Teste de Create PO a partir do hand-off selado Award, preservando exatamente os itens da cotação fonte; a decisão PO é total para esse tipo validado e não introduz rejeição nova.
3. Teste de metadados confirmando Close e Award independentes e Create PO dependente de Award.
4. Guard estrutural que impede Providers, state ports, FACTSTR ou operações de escrita nos adaptadores.
5. `cargo fmt`, `cargo check`, Clippy e testes de library; sem Testcontainers ou mutation testing.
