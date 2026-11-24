# Stage 5C — Reactor: handoff e discovery gate

**Checkpoint de origem:** `dc2bf33fbd6e95f0fdbaad77b3f8c644af794f93`
**Branch:** `feat/amri-stage4-three-prepared-participants`
**Estado:** `DISCOVERY_REQUIRED — NOT_READY_FOR_PRODUCTION_IMPLEMENTATION`
**ADR:** ADR-0034 permanece Proposed.

## 1. Objetivo desta retomada

Não começar criando `RfqSourcingReactor`. Primeiro comprovar que existe uma interação real da Application/Delivery que exige preparar consequências de pelo menos três capabilities independentes e confirmar todos os fatos relevantes em um único commit atômico. Sem essa prova, a ação correta é produzir um blocker report e parar, sem API, endpoint, façade, scaffolding ou código morto.

## 2. Inventário observado na branch de origem

| Fonte | Estado observado | Consequência |
|---|---|---|
| `src/app/amri/mod.rs` | Exporta `execution_context`, `rfq_sourcing_context` e Providers read-only; não exporta Reactor | Boundary de Application existente, mas sem executor de interação |
| `src/app/amri/execution_context.rs` | Factory `RfqSourcingExecutionContextFactory`, captura Clock/Id e fingerprint | Reutilizar contexto; não criar identidade paralela |
| `src/app/amri/rfq_sourcing_context.rs` | `composite_context_specification` e `project_snapshot` para os três participantes | Base de leitura/projeção de um único snapshot |
| `src/app/amri/providers/mod.rs` | Ports síncronos read-only ERP Reference, Compliance e Tax, com Fakes | Preservar distinção `Found/NotFound/NotApplicable` vs falhas operacionais; sem retry/failover automático |
| `src/app/atomic_sourcing_flow.rs` | Fachada para `FinalizeRfqSourcing` | Não é Reactor AMRI |
| `src/http/handlers.rs` | Handler `finalize_rfq_sourcing` chama `AtomicSourcingFlow` | Call-site atual pertence à capability única ADR-0031; não reaproveitar artificialmente |
| `docs/flows/atomic-rfq-sourcing-amri.md` | Cenário-alvo declarado Proposed/not implemented | Hipótese de design, não prova de consumidor de produção |

## 3. Discovery gate obrigatório

A próxima análise deverá responder com evidências no código e com validação do domain owner:

1. Qual é a interação concreta iniciada por usuário/sistema? Identificar comando, request, response e ponto de entrada real da Delivery.
2. Quais são as três capabilities independentes? Para cada uma, identificar owner, RPU autônomo existente, Functional Core reaproveitável e fatos propostos.
3. Qual invariante de negócio exige que os fatos das capabilities sejam all-or-none? Por que um único RPU/capability composta não é solução suficiente?
4. Qual o call-site real da Application que selecionará a interação? Se não existir, qual requisito aprovado autoriza criá-lo? Não inventar endpoint apenas para consumir Reactor.
5. Confirmar se o Create Purchase Order é capability independente ou consequência da capability de sourcing; justificar pela ownership do domínio.
6. Resolver a discrepância do DAG: o código Stage 4B declara Close e Award independentes, enquanto o diagrama proposto em `docs/flows/atomic-rfq-sourcing-amri.md` mostra Close → Award. Não adicionar aresta sem regra de domínio explícita.
7. Confirmar os Providers realmente necessários e quais regras consomem suas evidências; Provider é evidência, não policy nem atomicidade externa.

### Critério de parada

Se não for identificado requisito real, pelo menos três capabilities genuinamente independentes e call-site legítimo, registrar `STAGE5C_BLOCKED_NO_REAL_CONSUMER`, listar evidências consultadas e recomendar não implementar Reactor. Não preencher a lacuna com `FinalizeRfqSourcing`, `AtomicSourcingFlow` ou endpoint de demonstração.

## 4. Invariantes de implementação, somente após gate aprovado

- Consumir a `InteractionDefinition` estática da Stage 4B e usar ordem topológica estável.
- Close e Award continuam independentes conforme o código publicado e observam projeções do mesmo snapshot; Create PO depende do hand-off `AwardedSupplierQuote`, até que decisão explícita e justificada altere o grafo.
- Uma leitura lógica de snapshot; projeções tipadas de `CompositeSnapshot`; repassar token opaco sem transformação.
- `InteractionId`, fingerprint, timestamps e demais valores de execução capturados uma vez por execução lógica. CCC retry permitido conserva identidade/contexto lógico, obtém snapshot/token novos e descarta resultados preparados antigos.
- Preparação não persiste; não chamar `append`, `append_if`, commit port, event store ou SQL diretamente.
- Providers síncronos/read-only. Não executar Provider sob transaction/lock. Sem retry automático/failover.
- Falha de participante, dependency, Provider, evidência stale, cancelamento ou timeout aborta a preparação inteira.

## 5. Fronteiras que não podem ser antecipadas

- Stage 5C: coordenação/preparação somente. Não introduzir Composition Policies cross-capability nem sealed commit plan — Stage 6.
- Não escrever, chamar `append_if`, criar Interaction Marker ou implementar recovery — Stage 7.
- Não iniciar criação efetiva de purchase order/ERP; dispatch externo é pós-commit via outbox ou mecanismo durável definido depois.
- Não alterar RPUs autônomos nem `FinalizeRfqSourcing` ADR-0031.
- Não promover ADR-0034 para Accepted.

## 6. Testes e gates após discovery aprovado

- Ordem topológica determinística, grafo inválido, missing participant/dependency e hand-off Award → PO.
- Um snapshot para Close/Award; ambos observam estado pré-interação idêntico.
- Captura única Clock/Id e reuso do mesmo ExecutionContext em retry lógico.
- Participant rejection, Provider `Unavailable`/`TimedOut`/`InvalidResponse`, NotFound/NotApplicable, evidence stale, cancelamento e timeout.
- Qualquer erro aborta lote de preparação; fake commit/write port deve receber zero chamadas.
- Boundary guard confirma ausência de storage/write/provider no AMRI Core e Functional Core.
- Gates exatos serão definidos somente após conhecer o call-site e o escopo real; não rodar mutation em shell/Provider/adapter/Testcontainers.

## 7. Entregáveis da próxima sessão

1. Discovery report com paths, símbolos, call-sites e ownership.
2. Decisão documentada: consumidor real aprovado ou `STAGE5C_BLOCKED_NO_REAL_CONSUMER`.
3. Se aprovado: allowlist, ADR/decision impact, interface de entrada da Application e plano de testes antes do patch.
4. Se bloqueado: blocker report e encerramento da análise sem código de Reactor.

**Primeiro passo:** revisar este handoff junto de `src/app/amri/*`, `src/app/atomic_sourcing_flow.rs`, `src/http/handlers.rs`, `docs/flows/atomic-rfq-sourcing-amri.md`, `src/amri_participants/rfq_sourcing.rs` e ADR-0034 no HEAD atual.
