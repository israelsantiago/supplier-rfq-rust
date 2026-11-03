# Stage 5 — Reactor e Providers externos

**Estado:** Pendente. Só iniciar após Stage 4B aprovada, commitada e publicada.

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
