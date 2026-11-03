# Stage 4A — Shared Functional Core RFQ

**Estado:** Implementada, verificada pelo mantenedor e publicada em 2026-11-03.\
**Commit:** `203589e4d2c23338c8040e7802fabc77203fd6a9`.\
**Parent:** `7b004771cc1f42ab5def3cbda1c62bff50a92ff2`.

## Objetivo

Extrair as decisões puras realmente compartilhadas pelos três participantes e manter a mesma regra nas capabilities autônomas existentes.

## Arquivos Stage 4A

Tracked modificados:

- `src/lib.rs`
- `src/capabilities/close_rfq/build_context.rs`
- `src/capabilities/close_rfq/generate_consequences.rs`
- `src/capabilities/finalize_rfq_sourcing/decision.rs`

Novos:

- `src/domain/mod.rs`
- `src/domain/rfq_lifecycle.rs`
- `src/domain/sourcing.rs`

## Decisões e comportamento

- `RfqLifecycleContext` modela estado opened/closed com campos encapsulados.
- `decide_close_rfq` produz `RfqClosedFact` ou `RfqLifecycleRejection`.
- `SourcingContext`, `AwardQuoteSelection`, `AwardedSupplierQuote` e `SourcingRejection` apoiam as decisões de award/PO.
- `AwardedSupplierQuote` encapsula award fact e cotação fonte validada com suas linhas.
- `decide_award_supplier_quote` valida seleção, fornecedor, elegibilidade, validade, currency, entrega, itens e valores.
- `decide_create_purchase_order` deriva PO a partir do hand-off selado.
- `FinalizeRfqSourcing::decide` conserva uma única assinatura pública, sua ordem de rejeições e as quatro consequências históricas.
- `CloseRfq` autônomo delega ao shared decision e traduz rejeições para o erro legado; shell/state/CCC continuam existentes.
- `src/domain` estava registrado como `pub(crate)` no commit Stage 4A.

## Correção do `dead_code`

A primeira verificação encontrou getters `is_opened`/`is_closed` usados apenas em teste. A solução foi remover API que não tinha uso de produção e substituir teste de campos internos por teste de resultados observáveis via `decide_close_rfq`. Não introduzir `#[allow(dead_code)]` nem reduzir `-D warnings`.

## Evidência

O mantenedor informou aplicação e verificação V2 completas com sucesso. O GitHub confirma o commit e o diff exato de sete arquivos. Preservar a saída local dos comandos/CI no próximo update de evidências, se disponível.

## Critério de saída

Shared decisions compõem as capabilities originais, contratos legados preservados, gates fmt/check/tests/Clippy aprovados, branch commitada e publicada. Atendido conforme declaração do mantenedor e publicação remota.
