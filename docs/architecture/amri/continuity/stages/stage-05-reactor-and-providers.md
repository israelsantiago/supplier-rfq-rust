# Stage 5 — Reactor e Providers externos

**Estado:** Em andamento — Stage 5A. Stage 4B está publicada e os dez gates locais foram aprovados pelo Audit V3 em 2026-11-04.

## Subetapas executáveis da Stage 5

As subetapas abaixo são change sets de implementação, não novas fases do roadmap:

- **5A — Read-only Context Boundary e Projection:** restringir a capacidade de leitura, compor os requisitos dos três participantes e projetar os dois contextos do mesmo snapshot.
- **5B — Providers e Execution Context:** reutilizar `ClockProvider`/`IdProvider`, definir identidade/fingerprint de requisição e Providers externos read-only com Fakes.
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
