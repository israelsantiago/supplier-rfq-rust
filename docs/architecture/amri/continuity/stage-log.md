# Registro cronológico das fases e sessões

Este arquivo é o changelog de execução arquitetural. Não substitui Git log, CI ou evidências de teste.

## 2026-10-30 — Stage 1: design inicial

- Definida proposta AMRI com ADR-0034 e AMRI-D01..D16.
- Estabelecidas fronteiras entre RPU autônomo, participant preparado, Reactor, context planner, policies e state port.
- ADR-0034 permaneceu Proposed.
- Documentos existentes: `baseline.md`, `protocol.md`, `consistency-model.md`, `applicability.md`, `implementation-plan.md` e fluxos/diagramas.
- Evidências históricas descritas no `implementation-plan.md`; não repetir como execução desta sessão.

## 2026-10-30 — Stage 2: AMRI Core

- Adicionadas primitivas puras em `src/amri/`: identities, interaction definition, execution context, prepared outcomes, proposed facts, validação estrutural, DAG estático e validated plan.
- Sem RFQ/FACTSTR/SQLx/Postgres/Axum/async obrigatório no Core.
- Sem participants, Reactor, persistence ou Testcontainers nesta fase.
- Evidências históricas e comandos constam no plano original.

## 2026-11-01 — Stage 3: composite context/state port

- Implementados requirements e composição determinística da leitura, snapshot envelope e token opaco.
- Adaptador FACTSTR preserva query/token da leitura para append condicional.
- Sem Reactor, participants, composition policy, marker ou recovery.
- Semântica FACTSTR 0.5.2 (OR entre filtros, AND dentro de predicates específicos, versão do contexto) registrada nos docs da Stage 3.
- Evidências históricas constam em `stage-3-composite-context-state-port.md` e `implementation-plan.md`.

## 2026-11-03 — Stage 4A: Shared Functional Core

- Base: `7b004771cc1f42ab5def3cbda1c62bff50a92ff2`.
- Implementadas decisões compartilhadas em `src/domain/rfq_lifecycle.rs` e `src/domain/sourcing.rs`.
- Reutilização no `CloseRfq` e `FinalizeRfqSourcing` sem alterar suas shells/contratos públicos existentes.
- A primeira validação detectou `dead_code` nos métodos `is_opened`/`is_closed`; removidos e o teste foi alterado para verificar os resultados observáveis via `decide_close_rfq`.
- O patch V2 corrigiu a linha em branco adicional que fazia `rustfmt --check` abortar.
- Mantenedor informou que aplicação e verificação completa da Stage 4A V2 concluíram com sucesso.
- Commit publicado: `203589e4d2c23338c8040e7802fabc77203fd6a9` — `feat(amri): extract shared Functional Core for Stage 4A`.
- GitHub comparou o commit com seu pai: 1 à frente/0 atrás e 7 arquivos esperados.

## 2026-11-03 — Stage 4B V1: dry-run bloqueado

- Patcher V1 desenhou 3 participantes e uma nota de design; allowlist: `src/lib.rs`, `src/amri_participants/mod.rs`, `src/amri_participants/rfq_sourcing.rs`, `docs/architecture/amri/stage4b-prepared-participants.md`.
- Participantes reutilizam decisões Stage 4A; nenhuma state port/provider/persistência.
- `CloseRfqParticipant` e `AwardSupplierQuoteParticipant` independentes; `CreatePurchaseOrderParticipant` depende do Award.
- Dry-run abortou em 2026-11-03 com `rustfmt --check recusou o conteudo gerado`; output também mostrou ausência de `amri.rs` e `rfq_sourcing.rs` durante resolução de módulos temporários.
- Patcher disse `[ABORT] ... nenhum arquivo foi alterado`; portanto V1 não foi aplicada.
- Próxima correção V2 documentada em `current-state.md` e `stage-04b-prepared-participants.md` (arquivo individual de fase abaixo).

## Histórico — checkpoint documental planejado antes da Stage 4B (superado)

O mantenedor pediu consolidar em Markdown as decisões, arquitetura, plano por fases, referências, acordos e instruções de handoff. Naquele momento, o pacote de continuidade ainda precisava ser publicado antes da aplicação V2. Esse estado foi superado pela publicação do código Stage 4B no commit `5d278958855e41277bd00573d2cf419a50ee875d`; o estado operacional vigente está em `current-state.md` e `chat-handoff.md`.

## Modelo de registro para próximas fases

Copie o bloco em `quality-and-evidence.md` para cada fase e atualize a data, SHA, logs, CI, arquivos, backup e limites. Não registrar um comando como PASS se não foi executado ou se o retorno não foi visto.


## 2026-11-03 — Stage 4B V2: correção do fluxo rustfmt

O primeiro desenho de correção V2 foi revisto antes de ser entregue: `skip_children` é uma opção instável do rustfmt e não deve ser pressuposta no toolchain estável do projeto. A implementação final do patcher evita essa dependência. Ela formata e verifica apenas `rfq_sourcing.rs` como arquivo isolado, preserva a configuração rustfmt existente, valida as declarações triviais de módulo estruturalmente e deixa a validação completa da árvore para `cargo fmt --all -- --check` após a aplicação.

Os seis testes do patcher Stage 4B e os dois testes do instalador documental passaram neste empacotamento. Os mocks verificam que a formatação ocorre antes do `--check`, que nenhuma opção instável é passada ao rustfmt, que o diff usa o conteúdo formatado, que as 22 páginas Markdown e os dois índices são gerados, e que links relativos quebrados são rejeitados. Este ambiente não tem o binário real `rustfmt`, `cargo` ou `rustc`; o dry-run real e os gates Rust continuam pendentes no Ubuntu do mantenedor. Nenhum arquivo do repositório do usuário foi alterado pela falha V1.

## Histórico — correção do instalador de continuidade (antes da publicação Stage 4B)

- Resultado observado: dry-run documental V1 abortou por falso positivo nas duas páginas AMRI já existentes. Nenhum arquivo do repositório foi alterado.
- Causa: checagem de colisão não distinguia append gerenciado de sobrescrita.
- Correção V2: valida prefixo preservado + exatamente um marcador inicial/final; conteúdo existente após marcador precisa ser byte a byte idêntico; novos documentos divergentes continuam fail-closed.
- Evidência incluída no pacote: teste de append sobre README/plan existentes, segunda execução idempotente, detecção de marker drift e validação de links.
- Situação naquele momento: aguardar dry-run V2 e publicação documental antes da aplicação Stage 4B. Esse estado foi superado pelo commit `5d278958855e41277bd00573d2cf419a50ee875d`.

## Histórico — status do patcher Stage 4B V2 antes da publicação (superado)

- Os seis testes do patcher Stage 4B V2 passaram no empacotamento (incluindo ordem de formatação e verificação com mock). O dry-run real desse patcher não foi executado nesta sessão porque o checkpoint documental ainda não foi instalado/publicado.
- A tentativa de instalar a continuidade V1 abortou antes de escrever por falso positivo nos arquivos README/implementation-plan existentes.
- A continuidade V2 (pacote V3) corrige a colisão: valida append gerenciado, preservação de conteúdo, idempotência e marker drift; o restante do bundle é instalado em diretório novo e links relativos são verificados.
- Estado registrado naquele momento: Stage 4A publicada; Stage 4B ainda não aplicada; Stage 5–9 pendentes. Foi superado pela publicação Stage 4B no commit `5d278958855e41277bd00573d2cf419a50ee875d`.

## 2026-11-03 — Stage 4B: prepared participants publicados

- Código publicado na branch `feat/amri-stage4-three-prepared-participants`, commit `5d278958855e41277bd00573d2cf419a50ee875d` (`feat(amri): add prepared RFQ sourcing participants (Stage 4B)`).
- Participantes presentes: `CloseRfqParticipant`, `AwardSupplierQuoteParticipant` e `CreatePurchaseOrderParticipant`.
- Grafo: Close e Award independentes; Create PO depende do hand-off validado `AwardedSupplierQuote`.
- O escopo publicado é de preparação em memória e fatos propostos; não declara Reactor, Providers, composition policies, commit plan, `append_if`, marker ou recovery implementados.
- A consulta ao GitHub nesta revisão não encontrou status de commit nem workflow CI associado a esse SHA. Portanto, a publicação do código está confirmada; a verificação local de Rust/Cargo/Clippy deve ser registrada separadamente com logs reais do mantenedor.
- ADR-0034 permanece Proposed. Stage 5 — Reactor e Providers — é o próximo estágio planejado, ainda não implementado.


## 2026-11-04 — Stage 4B verificada localmente e início da Stage 5A

- O mantenedor executou `amri-stage5-continuity-audit-v3.sh` no checkout local.
- Resultado apresentado: baseline PASS; integridade do handoff e snapshot PASS; 10 gates PASS; 0 gates failed/blocked; 0 arquivos inesperadamente vazios.
- Pacote de auditoria V3 SHA-256: `3e50a879c1d3b1c911b73bcb54268911620f1066d755dbb55424b6ee627199d2`.
- A Stage 4 está concluída para fins de implementação e não deve ser reaberta sem evidência de regressão.
- Início da Stage 5 pelo change set 5A: read-only context boundary e projeção de contextos usando um único snapshot.
- A Stage 5 ainda não possui Reactor completo, Providers externos, cancelamento/timeout integrados ou coordenação completa dos três participantes.
- Nenhuma composition policy Stage 6 ou operação de commit Stage 7 foi introduzida.

## 2026-11-16 — Stage 5B.1: toolchain, CI parity, identity e ExecutionContext (planejado para aplicação)

- Baseline remoto confirmado: `921f60c4bb692cf8466e4a7e553c91dcf9bab923` na branch `feat/amri-stage4-three-prepared-participants`.
- Audit Stage 5B V2 local: seis gates PASS usando Rust/Cargo 1.99.1; o pacote reportou 212 testes aprovados. Essa toolchain não reproduziu integralmente o lint do CI.
- Falha CI reportada pelo mantenedor: Clippy 1.100.0 rejeitou o range/index no teste `rejects_sum_overflow_across_valid_line_totals` em `src/capabilities/submit_supplier_quote/decision.rs` com `clippy::needless_range_loop` e `-D warnings`.
- Correção planejada: `iter_mut().zip(...)` com assertions de cardinalidade do fixture; nenhum `allow` de lint.
- Fixar Rust/Cargo/Clippy/Rustfmt 1.100.0 em `rust-toolchain.toml` e em ambos os jobs estáveis; manter cobertura em nightly explícito.
- Adicionar `RfqSourcingExecutionContextFactory` na Application e fingerprint canônico SHA-256 v1 para `FinalizeRfqSourcingRequest`, sem alterar o AMRI Core genérico.
- Este registro descreve o change set planejado; não significa que Apply, gates, commit ou push tenham sido executados.

<!-- AMRI-STAGE5B2-STAGE-LOG:START -->

## 2026-11-17 — Stage 5B.2: design e pacote de implementação preparados

- Baseline remoto confirmado: `ae25951ddfa360c57946dd0b1ad47ba629ac6225` na branch `feat/amri-stage4-three-prepared-participants`.
- Auditoria de código identificou somente `supplier_id`, referências internas e metadados atuais; não identificou schema fiscal, veredicto de compliance externo ou contrato de integração ERP real.
- O change set define contratos provisórios de test double, não especificações de fornecedores externos.
- `wiremock` Rust 0.6.5 será executado num processo Rust em container próprio via Testcontainers `GenericImage`; o teste integrado fica fora da suíte `--lib`.
- Fakes ficam em módulo de suporte de teste; AMRI Core e Functional Core permanecem livres de HTTP/Providers/Testcontainers.
- Este registro descreve o pacote preparado. Apply, gates, Commit, Push e CI permanecem pendentes até saída observada.
<!-- AMRI-STAGE5B2-STAGE-LOG:END -->

<!-- AMRI-STAGE5B3R-STAGELOG:START -->

## 2026-11-20 — Stage 5B.2 re-verificação final

- O mantenedor executou a re-verificação final no HEAD
  `5fc082aee7e869908444b1bd6ac40d0df5c1721d`.
- Resultado do recibo: 17/17 gates PASS, zero FAIL.
- Onze gates executados novamente; seis resultados históricos reutilizados
  após validação de integridade.
- Evidência publicada no commit
  `511fa159ecdcdb4f7c81eddda7c47be5d1474f9f`.
- Mutation SKIPPED por política; promoção da fase não realizada.
- Próxima atividade: Stage 5B.3-R documental e handoff Stage 5C.

<!-- AMRI-STAGE5B3R-STAGELOG:END -->

<!-- AMRI-STAGE5B3R-STAGELOG-20261121:START -->

## 2026-11-21 — recuperação da publicação e fechamento documental em andamento

### Commit de guardrails publicado

- Commit: `dc2bf33fbd6e95f0fdbaad77b3f8c644af794f93`.
- Mensagem: `docs(amri): record shell isolation and executable runner guardrails`.
- Arquivos: `engineering-learnings.md`, `engineering-guardrails.md`.
- A aplicação foi validada com backup e checksums; auditoria pós-Apply: PASS.
- Precommit: 10/10 PASS; recibo recuperado do commit: 11/11 PASS.
- Push fast-forward: `164d51b..dc2bf33`; remoto verificado no mesmo SHA; sem force push.
- A falha inicial do publisher decorreu da ausência do commit receipt depois de o commit já ter sido criado. O runner de recuperação verificou parent, tree, allowlist, blobs, patch revisado e remoto antes de reconstruir evidência e publicar.
- O diagnóstico inicial também falhou ao acessar `COMMIT_RECEIPT_VALID` não exportado no ambiente Python. O defeito do diagnóstico foi identificado; essa execução não deve ser relatada como PASS.

### Revisão Stage 5B.2

- Recibo publicado: 17/17 PASS, 0 FAIL; 11 frescos e 6 históricos reutilizados; 12 registros históricos validados por tamanho/SHA-256.
- Mutation SKIPPED por política; CI remoto não confirmado; promoção automática não executada.
- Revisão da evidência concluída; manter limitações no status, sem transformar SKIPPED em PASS.

### Stage 5B.3-R

Este change set consolida estado, plano, decision register, quality/evidence, log, README e chat-handoff, e cria `stages/stage-05c-handoff.md`. Só marcar 5B.3-R como concluída após Apply/gates/commit/push confirmados.

<!-- AMRI-STAGE5B3R-STAGELOG-20261121:END -->
