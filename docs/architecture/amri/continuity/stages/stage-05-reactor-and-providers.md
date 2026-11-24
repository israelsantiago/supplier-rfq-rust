# Stage 5 — Reactor e Providers externos

<!-- AMRI-STAGE5-STATUS-20261121:START -->

> **Checkpoint 2026-11-21:** Stage 5B.2 possui evidência final revisada (17/17 PASS, com seis gates históricos reutilizados e mutation SKIPPED por política; CI remoto não confirmado). Stage 5B.3-R está em reconciliação documental final. Stage 5C entra primeiro em discovery gate: a branch `dc2bf33` ainda não demonstra consumidor real do Reactor. Consultar [handoff dedicado da Stage 5C](./stage-05c-handoff.md). Não antecipar código sem esse consumidor.

<!-- AMRI-STAGE5-STATUS-20261121:END -->


**Estado:** Stage 5B.2 re-verificada com 17/17 gates PASS; evidência-only publicada. Encerramento sujeito a revisão humana. Stage 5B.3-R documental; Stage 5C é a próxima implementação funcional.

## Subetapas executáveis da Stage 5

As subetapas abaixo são change sets de implementação, não novas fases do roadmap:

- **5A — Read-only Context Boundary e Projection:** restringir a capacidade de leitura, compor os requisitos dos três participantes e projetar os dois contextos do mesmo snapshot.
- **5B — Providers e Execution Context:** reutilizar `ClockProvider`/`IdProvider`, definir identidade/fingerprint de requisição e Providers externos read-only com Fakes.
  - **5B.1 — Toolchain/CI parity, Clippy e RFQ sourcing ExecutionContext:** fixar Rust 1.100.0, corrigir `needless_range_loop`, criar fingerprint canônico versionado e factory Application para `ExecutionContext`.
  - **5B.2 — External Read-only Providers:** contratos tipados e Fakes de ERP Reference, Compliance e Tax.
  - **5B.3 — Application Execution Context:** integrar os providers externos e preservar identidade/metadata na execução lógica; sem Reactor ainda.
- **5C — Reactor:** executar os participantes em ordem topológica estável, preservar o hand-off `AwardedSupplierQuote` e tratar abort/cancelamento/timeout.
- **5D — Hardening e fechamento:** completar matriz de testes, verificadores, evidências, documentação e revisão do diff.

Cada subetapa exige dry-run, backup, allowlist, diff e gates próprios. Não antecipar Stage 6 ou Stage 7.

## Objetivo

Construir coordenação determinística da interaction RFQ sem mover regra de negócio para o Reactor, sem acesso direto ao storage e sem side effects durante preparação.

## Escopo

- Definir o limite de aplicação para `RfqSourcingReactor`/interaction executor.
- Consumir a `InteractionDefinition` estática declarada na Stage 4B.
- Adquirir um único `SnapshotEnvelope` e token usando Stage 3 `AtomicInteractionStatePort`.
- Projetar tipos/contextos para participantes; Close e Award recebem projeções do mesmo snapshot.
- Executar sequencialmente na ordem topológica estável; Create PO recebe output de Award.
- Capturar InteractionId, ExecutionContext, timestamp(s) e IDs de fatos uma vez por execução lógica via shell/providers; reusar em retry permitido.
- Criar interfaces/Fakes para external read-only Providers (ex.: referência ERP e compliance/tax), sem integrar endpoints reais.
- Definir abort, erro, cancelamento e timeout antes de commit.

## Não fazer ainda

- Não executar composition policies cross-capability além dos checks locais dos participantes (Stage 6).
- Não selar/autorizar plano de commit final (Stage 6).
- Não fazer `append_if`, marker, recovery ou commit (Stage 7).
- Não realizar criação efetiva de pedido no ERP; isso será pós-commit via dispatch/outbox.

## Testes

- Ordem topológica determinística e hand-off Award → PO.
- Close/Award preparados a partir do mesmo snapshot; verificar que não há segunda leitura.
- Missing participant/dependency, participant rejection, provider timeout/unavailable, stale evidence, cancellation.
- Qualquer erro aborta toda a preparação; fake commit port não é chamado.
- IDs/timestamps permanecem estáveis para a mesma execução lógica conforme política definida.
- Providers não chamados dentro de transação/lock de store.

## Gates de saída

Reactor coordena, não decide; sem store access e sem writes; Fakes cobrem falhas/ordem; código e dependências respeitam fronteiras. Commit/push separados e documentação atualizada.

## Contrato específico 5B.1 — identidade e metadados

- Definition ID: `supply-chain.rfq-sourcing`; definition version: `1`. Os dois valores têm fonte única em `amri_participants::rfq_sourcing` e visibilidade `pub(crate)`, sem ampliar a API pública.
- Fingerprint: SHA-256 v1 com domain separator ASCII `AMRI/RFQ-SOURCING/FINALIZE-REQUEST` seguido de byte NUL (`0x00`); schema `u16` big-endian; definition version `u32` big-endian; UUIDs como 16 bytes em ordem de rede; quote-set sequence `u64` big-endian. A ordem dos campos é `rfq_id`, `supplier_id`, `quote_id`, `expected_quote_set_version`.
- O fingerprint inclui `rfq_id`, `supplier_id`, `quote_id` e `expected_quote_set_version`; não inclui timestamp, correlation ID ou InteractionId.
- `InteractionId` é gerado pela Application via `IdProvider`; `captured_at_unix_ms` é capturado via `ClockProvider`.
- O `ExecutionContext` resultante é imutável e deve ser conservado/reutilizado em retries da mesma execução lógica. Invocar a factory novamente significa iniciar outra interação e gera novo ID/timestamp.
- O AMRI Core continua recebendo valores concretos e não conhece os Providers nem o encoder/hash.

<!-- AMRI-STAGE5B2-STAGE-DOC:START -->

## Stage 5B.2 — External Read-Only Providers

**Estado:** change set delimitado e pacote preparado; Apply e Verify no checkout do mantenedor pendentes.

### Contratos Application

Os contratos ficam em `src/app/amri/providers/`, dentro da Application e com visibilidade `pub(crate)`. Os três ports são síncronos, recebem `Uuid supplier_id` já presente no domínio e devolvem referências tipadas com proveniência.

`ProviderLookup<T>` distingue `Found`, `NotFound` e `NotApplicable`. `ExternalProviderError` distingue `Unavailable`, `TimedOut` e `InvalidResponse`. `ProviderEvidenceMetadata::freshness_at(instant)` classifica somente a expiração fornecida pela fonte (`Fresh`, `Stale` ou `UnboundedBySource`); não infere max-age e não rejeita a evidência. A política de consumo permanece fora desta etapa.

### Fakes

`FakeErpReferenceProvider`, `FakeComplianceProvider` e `FakeTaxProvider` são determinísticos e compilados somente para `cfg(test)` ou feature `test-support`. Não usam rede, timers, Docker, PostgreSQL ou Testcontainers.

### Contratos HTTP de teste

`tests/contracts/external-providers-v1.openapi.yaml` define schemas limitados a referências opacas e proveniência. O test double não representa API oficial do ERP, Compliance ou Tax de qualquer fornecedor.

`tests/wiremock-rs/` empacota `wiremock` Rust 0.6.5 numa imagem local. `tests/external_provider_contracts.rs` inicia essa imagem por Testcontainers; o alvo é integrado e exige Docker. O build ocorre por `scripts/build-amri-wiremock-rs-image.sh` antes de `cargo test --all-targets`.

### Fronteiras e exclusões

Providers não entram em AMRI Core, Functional Core ou Prepared Participants; não consultam Application State; não executam writes nem garantem atomicidade externa. Nenhuma regra de Compliance/Tax é decidida. Nenhum endpoint real/credencial/SDK é integrado. Não antecipar 5B.3, 5C, Stage 6 ou Stage 7.
<!-- AMRI-STAGE5B2-STAGE-DOC:END -->

<!-- AMRI-STAGE5B3R-STAGE5:START -->

## Continuidade após Stage 5B.2 — 2026-11-20

A Stage 5B.2 possui recibo final publicado com 17/17 gates PASS e zero FAIL.
O recibo registra 11 gates recém-executados, seis gates históricos reutilizados
após validação dos logs, mutation `SKIPPED_BY_STAGE5B2_POLICY` e
`PHASE_STATUS_PROMOTION=NOT_PERFORMED`.

A evidência não confirma CI remoto e não altera automaticamente o estado
da fase. O encerramento formal da 5B.2 permanece sujeito à revisão humana.

### Stage 5B.3-R

Subetapa documental. Atualiza continuidade, acordos, guardrails, evidências,
registro de decisões e handoff. Não cria produção, novos ports, testes
artificiais nem amplia a superfície pública.

### Stage 5C — primeiro consumidor Reactor

A implementação deverá integrar um consumidor real da Application, não
tratar `FinalizeRfqSourcing`/`AtomicSourcingFlow` do ADR-0031 como Reactor AMRI.

Invariantes de execução:

- Close e Award são independentes e observam projeções do mesmo snapshot.
- Create Purchase Order depende do resultado `AwardedSupplierQuote`.
- Ordem de execução é determinística e topológica.
- `InteractionId`, fingerprint e timestamps são capturados uma vez por
  execução lógica e preservados em retries CCC.
- Cada nova tentativa obtém snapshot/token atualizados; resultados preparados
  da tentativa anterior são descartados.
- Providers permanecem síncronos/read-only e preservam ausência de evidência
  separada de falhas operacionais.
- Não introduzir retry automático/failover de Provider.
- Nenhum participante ou Reactor executa persistência na Stage 5.

### Fronteiras posteriores

Stage 6: policies de composição e commit plan imutável.
Stage 7: único `append_if`, Interaction Marker e recovery.
ADR-0034 permanece Proposed até os critérios de aceitação posteriores.

<!-- AMRI-STAGE5B3R-STAGE5:END -->
