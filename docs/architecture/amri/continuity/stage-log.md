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
