# Referências e fontes de verdade

## 1. Documentos normativos do repositório

Para a lista completa dos 34 ADRs e o mapa de código/testes/scripts por Stage, consulte [source-map-and-adr-catalogue.md](./source-map-and-adr-catalogue.md).

| Documento | Papel |
|---|---|
| [`ADR-0034`](../../adr/0034-atomic-multi-rpu-interaction.md) | Fonte principal da proposta AMRI, AMRI-D01..D16, testes e critérios de aceitação. Status continua Proposed. |
| [`AMRI README`](../README.md) | Índice e status macro. |
| [`baseline.md`](../baseline.md) | Baseline e código que deve permanecer protegido. |
| [`protocol.md`](../protocol.md) | Papéis, contratos e separação Core/adapter. |
| [`consistency-model.md`](../consistency-model.md) | Snapshot, token, CCC, idempotência, conflito e falha. |
| [`implementation-plan.md`](../implementation-plan.md) | Roadmap original Stages 0–9. |
| [`stage-3-composite-context-state-port.md`](../stage-3-composite-context-state-port.md) | Prova Stage 3 de composite context/state port e FACTSTR. |
| [`applicability.md`](../applicability.md) | Quando AMRI se aplica e como generalizar. |
| [`ADR-0031`](../../adr/0031-atomic-rfq-sourcing-finalization.md) | `FinalizeRfqSourcing` como capability única com quatro fatos/um append. |
| [`ADR-0033`](../../adr/0033-application-state-provider-specialized-ports.md) | Contratos especializados de Application State. |
| [`ADR-0006`](../../adr/0006-command-context-consistency.md) | Consistência do contexto. |
| [`ADR-0010`](../../adr/0010-clock-and-id-providers.md) e [`ADR-0011`](../../adr/0011-stable-command-metadata-on-ccc-retry.md) | Providers e metadata estável por execução lógica. |
| [`ADR-0023`](../../adr/0023-no-cross-rpu-orchestrator.md) | Decisão histórica de não encadear shells autônomas no escopo anterior; AMRI continua opt-in/proposto. |

## 2. Rico Fritzsche — conceitos primários

A arquitetura AMRI usa esses conceitos para vocabulário e fronteiras. O protocolo preparado/commit AMRI é extensão específica deste projeto e não deve ser atribuído como contrato canônico ao autor.

- [Architecture Knowledge Base](https://architecture.ricofritzsche.me/)
- [Concepts — whole picture](https://architecture.ricofritzsche.me/concepts/)
- [Domain Capability](https://architecture.ricofritzsche.me/concepts/domain-capability/)
- [Commands and Queries](https://architecture.ricofritzsche.me/concepts/commands-and-queries/)
- [Request Processing Units](https://architecture.ricofritzsche.me/concepts/request-processing-units/)
- [Reactors](https://architecture.ricofritzsche.me/concepts/reactors/)
- [User Interactions](https://architecture.ricofritzsche.me/concepts/user-interactions/)
- [Delivery Mechanisms](https://architecture.ricofritzsche.me/concepts/delivery-mechanisms/)
- [Providers](https://architecture.ricofritzsche.me/concepts/providers/)
- [Functional Core / Imperative Shell](https://architecture.ricofritzsche.me/concepts/functional-core-imperative-shell/)
- [Application State](https://architecture.ricofritzsche.me/concepts/application-state/)
- [Application State Provider](https://architecture.ricofritzsche.me/concepts/application-state-provider/)
- [Command Context Consistency](https://architecture.ricofritzsche.me/concepts/command-context-consistency/)
- [Event Store API — CCC specification](https://architecture.ricofritzsche.me/specifications/command-context-consistency/event-store-api/)

## 3. Implementações e contratos externos

- [FACTSTR — repository](https://github.com/ricofritzsche/factstr)
- [FACTSTR Tool Rental Example — Rust](https://github.com/ricofritzsche/factstr-tool-rental-example/tree/main/rust)
- [Booking a Stay — Rust example](https://github.com/ricofritzsche/booking-a-stay-example/tree/main/rust-example)
- [Supplier RFQ — este projeto](https://github.com/israelsantiago/supplier-rfq-rust)
- [FACTSTR 0.5.2 — crates.io](https://crates.io/crates/factstr/0.5.2)
- [factstr-postgres — crates.io](https://crates.io/crates/factstr-postgres)

## 4. Rust e toolchain

- [The Rust Reference — visibility/privacy](https://doc.rust-lang.org/reference/visibility-and-privacy.html)
- [The Rust Reference — modules](https://doc.rust-lang.org/reference/items/modules.html)
- [The Rust Book — defining modules](https://doc.rust-lang.org/book/ch07-02-defining-modules-to-control-scope-and-privacy.html)
- [rustfmt documentation](https://rust-lang.github.io/rustfmt/)
- [rustfmt configuration](https://rust-lang.github.io/rustfmt/?version=v1.8.0&search=skip_children)
- [Rust 2024 Edition Guide](https://doc.rust-lang.org/edition-guide/rust-2024/)
- [Cargo Book](https://doc.rust-lang.org/cargo/)

## 5. Consistência relacional como contraste

- [PostgreSQL — Transaction Isolation](https://www.postgresql.org/docs/current/transaction-iso.html)
- [PostgreSQL — Explicit Locking](https://www.postgresql.org/docs/current/explicit-locking.html)

A existência de transações não deve ser confundida com prova automática de proteção do read set completo, phantom protection ou ausência de write skew. O adapter relacional hipotético precisa provar semântica equivalente através de isolamento/locks/constraints e testes concorrentes.

## 6. Chats e histórico conversacional usados como contexto

Títulos recuperáveis das conversas relacionadas:

- `Avaliação de Reatores e RPUs` — planejamento AMRI e entregas por etapas.
- `Próximos passos da Etapa 1` — retomada do plano incremental.
- `Verificação de aderência arquitetural` — boundaries, guards, mutation testing.
- `Avaliação arquitetural pragmática` — ClockProvider/IdProvider e estabilidade dos metadados.
- `Avaliação arquitetural DCB` — pesquisa de consistência e decisões/agregados.
- `Resolver bug Rust Cargo test` e `Avaliação arquitetural pragmática` — gates de teste e execução segura.

Este pacote não afirma ter importado literalmente todo o conteúdo desses chats. O estado consolidado foi derivado dos resumos de contexto disponíveis e dos documentos/código consultados na branch. Quando um detalhe vem de conversa e não do código/ADR, está marcado como decisão conversacional ou `reported by maintainer`.
