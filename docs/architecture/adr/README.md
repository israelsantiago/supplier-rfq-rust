# Architecture Decision Records (ADR) — Supplier RFQ Rust

> Catálogo das decisões arquiteturais, de design e de stack que explicam a implementação da branch `main`. Este catálogo distingue decisões aceitas das limitações adiadas; não declara o sistema pronto para produção.

## Como ler este catálogo

Cada ADR individual registra contexto, decisão e consequências. `Accepted` significa que a decisão descreve a implementação da branch `main`; `Deferred` significa que o tema é reconhecidamente necessário ou relevante, mas ainda não foi implementado/fechado.

| ADR | Decisão | Status |
|---|---|---|
| [ADR-0001](./0001-rust-platform.md) | Rust 2024 e plataforma de compilação | Accepted |
| [ADR-0002](./0002-axum-http-adapter.md) | Axum como adaptador HTTP | Accepted |
| [ADR-0003](./0003-domain-capabilities-rpu.md) | Domain Capabilities como unidades de processamento | Accepted |
| [ADR-0004](./0004-functional-core-imperative-shell.md) | Functional Core / Imperative Shell | Accepted |
| [ADR-0005](./0005-factstr-event-store.md) | FACTSTR como Event Store autoritativo | Accepted |
| [ADR-0006](./0006-command-context-consistency.md) | Command Context Consistency (CCC) | Accepted |
| [ADR-0007](./0007-versioned-immutable-facts.md) | Fatos imutáveis e tipos versionados | Accepted |
| [ADR-0008](./0008-money-integer-minor-units.md) | Valores monetários em unidades menores inteiras | Accepted |
| [ADR-0009](./0009-identity-and-client-supplied-ids.md) | Identidade por UUID e IDs explícitos opcionais | Accepted |
| [ADR-0010](./0010-clock-and-id-providers.md) | Providers explícitos para relógio e IDs | Accepted |
| [ADR-0011](./0011-stable-command-metadata-on-ccc-retry.md) | Metadados estáveis durante retries internos de CCC | Accepted |
| [ADR-0012](./0012-postgresql-storage.md) | PostgreSQL como persistência | Accepted |
| [ADR-0013](./0013-durable-stream-projection.md) | Projeção via stream durável e consistência eventual | Accepted |
| [ADR-0014](./0014-sqlx-application-owned-read-model.md) | SQLx para persistência da projeção da aplicação | Accepted |
| [ADR-0015](./0015-http-dto-boundary.md) | DTOs HTTP separados dos fatos internos | Accepted |
| [ADR-0016](./0016-test-strategy.md) | Testes unitários, propriedades e aceitação com PostgreSQL | Accepted |
| [ADR-0017](./0017-blocking-store-calls-on-tokio.md) | Isolar operações síncronas do FACTSTR em threads bloqueantes | Accepted |
| [ADR-0018](./0018-bounded-ccc-retry.md) | Retries CCC limitados e sem loops cegos | Accepted |
| [ADR-0019](./0019-eligibility-assignment-semantics.md) | Elegibilidade como atribuição de estado | Accepted |
| [ADR-0020](./0020-event-query-semantics.md) | Semântica explícita das EventQueries CCC | Accepted |
| [ADR-0021](./0021-api-security-deferred.md) | Autenticação e autorização fora do escopo da referência | Deferred |
| [ADR-0022](./0022-http-idempotency-deferred.md) | Idempotência de comandos HTTP | Accepted |
| [ADR-0023](./0023-no-cross-rpu-orchestrator.md) | Sem orquestrador transversal de RPUs no fluxo atual | Accepted |
| [ADR-0024](./0024-test-support-feature.md) | Seam de teste protegido por feature | Accepted |
| [ADR-0025](./0025-rpu-command-facade.md) | Fachada pública de comandos e encapsulamento de Providers | Accepted |

## Políticas não negociáveis nesta refatoração

- Não alterar regras de negócio, validações, códigos HTTP, nomes/versionamento de fatos ou a semântica das queries CCC nesta mudança.
- Manter a suíte existente e executar `cargo fmt`, `cargo check`, `cargo clippy` e `cargo test --all-features` antes do commit.
- O núcleo funcional recebe valores concretos; Providers são usados somente na Imperative Shell/composição.
- IDs e timestamps de ocorrência são escolhidos na shell uma vez por execução lógica; `CloseRfq` e `SubmitSupplierQuote` os mantêm estáveis em retries CCC internos. Commands sem retry interno retornam conflito em vez de refazer silenciosamente a decisão.
- Endpoints de comando podem usar `Idempotency-Key` para reproduzir a resposta de uma requisição HTTP repetida; sem a chave, não há essa garantia. O ledger não é atomicamente commitado junto ao FACTSTR e `in_progress`/`unknown` exigem reconciliação.
- Se a regra futura exigir validade no instante da aceitação, separar o instante de submissão (`submitted_at`) do instante de decisão/aceitação (`decision_at`/`accepted_at`) em ADR e alteração de domínio próprios.
- Os scripts de commit/push exigem opt-in explícito via `PUSH_TO_GITHUB=YES`.

## Referências primárias

- [Rico Fritzsche — Architecture Knowledge Base](https://architecture.ricofritzsche.me/)
- [Request Processing Units](https://architecture.ricofritzsche.me/concepts/request-processing-units/)
- [Functional Core / Imperative Shell](https://architecture.ricofritzsche.me/concepts/functional-core-imperative-shell/)
- [Providers](https://architecture.ricofritzsche.me/concepts/providers/)
- [Command Context Consistency](https://architecture.ricofritzsche.me/concepts/command-context-consistency/)
- [FACTSTR](https://github.com/ricofritzsche/factstr)
- [FACTSTR Tool Rental Example — Rust](https://github.com/ricofritzsche/factstr-tool-rental-example/tree/main/rust)
