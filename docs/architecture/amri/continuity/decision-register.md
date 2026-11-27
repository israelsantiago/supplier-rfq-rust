# Registro de decisões, escolhas e invariantes AMRI

<!-- AMRI-PROCUREMENT-COMMIT-DECISIONS-20261125:START -->

## Checkpoint vigente — piloto Procurement Commit (2026-11-25)

| ID | Decisão/candidato | Estado e limite |
|---|---|---|
| D-5C-03 | Interaction `supply-chain.procurement-commit` v1 | Protótipo Stage 5C implementado; Apply/Test/Precheck v1.0.3 reportados PASS. Change set corretivo v1.0.4 exige seus próprios gates. Não substitui ADR-0031. |
| D-5C-04 | Grafo | Reserve Budget e Award independentes; Issue PO depende de ambos. Execução sequencial/topológica e determinística; não inserir CloseRfq. |
| D-5C-05 | `procurement_budget_allocated.v1` | Contrato proposto, ausente do baseline. Owner/produtor nomeado ainda não identificado; não inventar ownership. Aprovação de Financeiro/Procurement é gate de produção. |
| D-5C-06 | `purchase_order_created.v2` | Contrato novo com `budget_id` e `budget_reservation_id`; v1 permanece integralmente preservado. |
| D-5C-07 | Consistência da preparação | Um `CompositeContextReader`, um snapshot, um token opaco; nenhuma porta de escrita no Reactor. |
| D-5C-08 | Identidade e retry | Capturar identidade/relógio/IDs uma vez; retries reutilizam identidade/IDs e fazem nova leitura; outcomes anteriores são descartados. |
| D-5C-09 | Fronteira de fases | Stage 5 prepara; Stage 6 valida estrutura/composição e sela plano imutável; Stage 7 executa o único `append_if`, marker e recovery. |
| D-5C-10 | Delivery | Sem endpoint, shell autónoma, call-site produtivo ou ERP nesta entrega. |
| D-5C-11 | CCC/DCB | O adapter usa event type + payload predicates; nenhuma compatibilidade nativa com tags DCB é reivindicada. |
| D-5C-12 | Política Procurement Budget v1 | Uma alocação imutável por `budget_id`; reserva positiva, moeda idêntica e saldo nunca negativo; uma reserva histórica por `(budget_id, rfq_id)`. Sem ajuste, release, cancelamento com restituição ou reserva substituta em v1. Formalizada em `procurement-budget-contract.md`. |
| D-5C-13 | Ownership do contrato de orçamento | Status `Proposed / owner approval required`. A fonte não identifica equipe/pessoa ou produtor da alocação; isso deve ser definido pelo owner de negócio antes do Stage 7/produção. Stage 6 pode avançar sem escrita, mantendo o bloqueio explícito. |
| D-5C-14 | Fingerprint retornado | `context_specification_fingerprint` identifica a especificação da leitura. Não é hash do snapshot nem token de consistência; o token opaco do adapter é preservado separadamente. |

### Gate de domínio do orçamento

A regra v1 fail-closed foi escolhida para o protótipo: alocação única/imutável; uma reserva histórica por par `(budget_id, rfq_id)`; sem release ou ajuste. Isso não cria uma política de negócio aprovada. Antes de Stage 7/produção, o owner Financeiro/Procurement deverá aprovar o produtor, a granularidade, alterações de alocação, cancelamento, release total/parcial, re-reserva e ciclo de vida da reserva. Nenhum release parcial ou implícito será inferido.

ADR-0034 permanece `Proposed`. Stage 6 está limitado à validação estrutural/políticas e commit plan imutável, sem persistência. Stage 7 e a ativação produtiva aguardam o gate do contrato de orçamento.

<!-- AMRI-PROCUREMENT-COMMIT-DECISIONS-20261125:END -->


**Legenda:** `Accepted` = decisão formal aceita em ADR; `Proposed` = decisão de ADR-0034 ainda em avaliação; `Implemented` = existe código; `Reported by maintainer` = execução confirmada pelo mantenedor, sem log completo arquivado aqui.

## 1. ADRs e decisões de base

| ADR | Estado conhecido | Decisão relevante para AMRI |
|---|---|---|
| ADR-0003 — Domain Capabilities/RPU | Accepted | Um RPU implementa uma capability e possui sua fronteira request/response; RPUs autônomos não chamam uns aos outros. |
| ADR-0004 — Functional Core / Imperative Shell | Accepted | Decisões puras, efeitos nas shells; valores externos entram explicitamente no Core. |
| ADR-0005 — FACTSTR | Accepted | Event Store como storage de fatos; writes condicionais fazem parte do contrato de consistência. |
| ADR-0006 — Command Context Consistency | Accepted | A decisão só persiste enquanto os fatos de seu contexto ainda forem válidos. |
| ADR-0010 — Clock/Id Providers | Accepted | Clock/IDs fornecidos por abstrações; Functional Core não acessa relógio nem aleatoriedade. |
| ADR-0011 — metadados estáveis em retry | Accepted | Capturar metadata uma vez por execução lógica; reutilizar ao re-preparar após CCC retry. |
| ADR-0016 — estratégia de testes | Accepted | Separar testes puros, shell/adapter e integração. |
| ADR-0023 — sem cross-RPU orchestrator no escopo anterior | Accepted para escopo anterior | Não encadear shells autônomas. AMRI é nova proposta opt-in e não revoga automaticamente o ADR. |
| ADR-0026 — Application State Provider | Accepted | Storage fica no adapter/port; decisão permanece no Core. |
| ADR-0031 — finalização atômica de sourcing | Accepted | Uma capability `FinalizeRfqSourcing` produz quatro fatos e um `append_if`; não é Reactor/AMRI. |
| ADR-0033 — portas especializadas ASP | Accepted | Preservar contratos específicos; não criar abstração genérica sem semântica compartilhada comprovada. |
| ADR-0034 — Atomic Multi-RPU Interaction | **Proposed** | Protocolo multi-capability. Etapas parciais podem estar implementadas sem aceitar o protocolo completo. |

## 2. AMRI-D01 a AMRI-D16 — síntese fiel do ADR-0034

Todos os itens abaixo continuam `Proposed` até que os gates específicos do ADR-0034 sejam cumpridos e a revisão formal altere o status.

| ID | Decisão proposta | Consequência operacional |
|---|---|---|
| AMRI-D01 | AMRI opt-in por interação | Só usar quando o requisito exige atomicidade entre capabilities distintas. |
| AMRI-D02 | Preservar RPU autônomo | `process(request) -> response` continua completo e independente; participante é adapter adicional. |
| AMRI-D03 | Preparação sem efeitos duráveis | Nenhum write/append, projeção ou side effect irreversível no prepare. |
| AMRI-D04 | Reutilizar o mesmo Functional Core | Sem duplicar regra nem chamar shell de outra capability. |
| AMRI-D05 | Declarar context requirements antes da leitura | Planner compõe semanticamente um único contexto; Core não conhece EventQuery/SQL. |
| AMRI-D06 | Um snapshot coerente e um token opaco | Token único protege o conjunto de fatos relevantes; nunca combinar tokens independentes. |
| AMRI-D07 | DAG explícito e ordem determinística | Primeiro executor sequencial/topológico; dependências são dados tipados, sem imports cruzados. |
| AMRI-D08 | Separar decisão local, validação estrutural e composição de domínio | Core verifica estrutura; policy do domínio verifica invariantes cruzadas; Reactor coordena. |
| AMRI-D09 | Providers fornecem evidência, não atomicidade distribuída | Usar reservation/lease quando exclusividade é necessária; dispatch após commit. |
| AMRI-D10 | Um único limite de commit autoritativo | Uma chamada por interaction port; adapter FACTSTR faz um único `append_if` com lote completo. |
| AMRI-D11 | Idempotência dentro do contrato atômico | InteractionId + fingerprint; marker proposto no mesmo batch; recuperação do resultado. |
| AMRI-D12 | Conflito aborta e exige re-preparação | Token stale invalida lote; retries só se semântica permitir, com nova leitura e novas decisões. |
| AMRI-D13 | Core genérico sem domínio/storage | Sem tipos RFQ, FACTSTR, SQLx, PostgreSQL, Axum, broker ou async obrigatório. |
| AMRI-D14 | Adapter prova conformidade semântica | Coerência, proteção do read set, all-or-none, concorrência, marker e recovery precisam de testes. |
| AMRI-D15 | Erros/observabilidade explícitos | Distinguir rejeição de negócio, composition, conflito, falha definitiva e commit ambíguo. |
| AMRI-D16 | Workflow durável fora da interação atômica inicial | Espera humana/processo longo exigem process manager/saga separado. |

## 3. Decisões específicas do projeto já consolidadas

### Sourcing atômico de uma capability

`FinalizeRfqSourcing` permanece como baseline independente: decisão com quatro consequências e um append condicional. Não a reclassificar como Reactor, AMRI ou três RPUs.

### Shared Functional Core (Stage 4A)

- `src/domain/rfq_lifecycle.rs` contém `RfqLifecycleContext`, `RfqLifecycleRejection` e `decide_close_rfq`.
- `src/domain/sourcing.rs` contém `SourcingContext`, `AwardQuoteSelection`, `AwardedSupplierQuote`, `SourcingRejection`, `decide_award_supplier_quote` e `decide_create_purchase_order`.
- Campos do contexto e de `AwardedSupplierQuote` permanecem encapsulados onde desenhado; hand-off só é criado pela decisão validada.
- A decisão legada `FinalizeRfqSourcing::decide` foi mantida com uma única assinatura pública, compondo funções puras compartilhadas e preservando os critérios/precedência históricos de rejeição.
- `CloseRfq` autônomo continua usando sua shell e Application State Provider original; só a decisão pura foi compartilhada.

### Stage 4B — Prepared Participants (implementada e publicada)

- Commit de implementação publicado: `5d278958855e41277bd00573d2cf419a50ee875d`.
- Participantes: `CloseRfqParticipant`, `AwardSupplierQuoteParticipant` e `CreatePurchaseOrderParticipant`.
- Cada participante reutiliza o Shared Functional Core e produz resultado/fato proposto sem persistência.
- Close e Award são independentes; Create PO consome o hand-off selado `AwardedSupplierQuote`.
- `InteractionDefinition` permanece metadado declarativo; não existe Reactor nesta etapa.
- A API é experimental e seletiva em `crate::amri_participants`; `crate::domain` não é promovido integralmente para público.
- Publicação não altera o estado da ADR-0034, que permanece Proposed, nem prova os gates locais de Rust. Consultar/registrar logs reais em `quality-and-evidence.md`.

### Metadata estável

`ClockProvider` e `IdProvider` são chamados na shell/coordenação; timestamps e IDs relevantes são capturados uma vez por execução lógica e passados ao Functional Core/participant. A alternativa de timestamp no instante de aceitação exige separar `submitted_at` de `accepted_at`/`decision_at` e revisar regras fiscais/comerciais; não é adotada implicitamente.

### Testes de mutação

Mutation testing é restrito a Functional Core puro, AMRI Core e políticas puras explicitamente selecionadas. O runner de mutação não pode iniciar ou exigir Docker, PostgreSQL, Testcontainers nem mutar adapter/shell/infraestrutura. A campanha consolidada foi reservada para depois de a implementação AMRI estar completa; não declarar mutação executada na Stage 4B.

## 4. Alternativas rejeitadas ou não selecionadas

- Chamar shells autônomas sequencialmente: rejeitado para atomicidade all-or-none porque cada shell pode confirmar antes da próxima falhar.
- Três `append_if` independentes: rejeitado; não há rollback do primeiro append se o posterior conflitar.
- Três reads e combinação de versões: rejeitado; não cria snapshot coerente.
- Um Core universal baseado em `serde_json::Value`: não selecionado; preservar tipos estáticos.
- Reactor com acesso direto a SQL/EventStore: rejeitado; usar porta de interação; adapter é dono do mecanismo.
- Transação distribuída com ERP: não presumir; external actions pós-commit ou reservation/compensação explícita.
- Motor BPMN/workflow geral: fora do escopo AMRI.
- Extrair crate genérico separado antes do segundo domínio: adiado até provar reuse.

## 5. Mudança de decisão exige trilha

Toda alteração de decisão precisa registrar: (a) estado anterior; (b) motivação/evidência nova; (c) ADR/decision ID afetado; (d) código, testes e documentos impactados; (e) compatibilidade/migração; (f) gate para aceitar/superseder. Não editar decisão apenas porque o script atual torna a implementação mais conveniente.

<!-- AMRI-STAGE5B2-DECISIONS:START -->

## Stage 5B.2 — decisões delimitadas (2026-11-17)

| Decisão de implementação | Estado | Limite |
|---|---|---|
| Portas ERP Reference, Compliance e Tax pertencem à Application | Proposta implementada no change set; gates pendentes | Não entram em AMRI Core/Functional Core |
| Respostas carregam referência opaca e proveniência | Contrato provisório de teste | Não define atributo comercial/fiscal nem verdictos de compliance |
| `NotFound`/`NotApplicable` diferem de indisponibilidade/timeout | Contrato tipado | Ausência não é falha de transporte ou rejeição de domínio |
| Fakes determinísticos são feature/test-only | Proposto no change set | Sem rede, sleeps, Docker ou Testcontainers |
| WireMock-RS 0.6.5 roda num container Rust customizado via Testcontainers | Harness de teste | Imagem local exclusiva de integração; não é serviço de produção nem imagem Java oficial |
| Classificação de expiração | Helper de proveniência no change set | `freshness_at` compara somente `expires_at` explícito; não aplica max-age nem decide rejeição. A política de consumo permanece não decidida |
| API de fornecedor real | Não decidida | OpenAPI da 5B.2 representa somente o test double |

ADR-0034 permanece `Proposed`. 5B.2 não autoriza Reactor 5C, composition policies Stage 6, commit Stage 7 ou integração real de ERP/Compliance/Tax.
<!-- AMRI-STAGE5B2-DECISIONS:END -->

<!-- AMRI-STAGE5B3R-DECISIONS:START -->

## Decisões operacionais consolidadas — 2026-11-20

- Stage 5B.3-R é documentation-only.
- Não adicionar produção sem consumidor Reactor real.
- ADR-0031 / `AtomicSourcingFlow` permanece fluxo legado independente.
- ADR-0033 permanece Accepted.
- ADR-0034 permanece Proposed.
- Stage 5 continua prepare/read-only; não inclui composição de domínio,
  commit plan, `append_if`, marker ou recovery.
- Stage 6 é responsável por composition policies e commit plan.
- Stage 7 é responsável pelo append condicional único, marker e recovery.
- CCC retries preservam identidade lógica, fingerprint e timestamps;
  renovam snapshot/token e descartam resultados preparados obsoletos.

<!-- AMRI-STAGE5B3R-DECISIONS:END -->

<!-- AMRI-STAGE5B3R-DECISIONS-20261121:START -->

## 5. Decisões operacionais e gates — 2026-11-21

### D-5B3R-01 — Falha de publisher não invalida automaticamente commit já criado

**Estado:** Accepted como regra operacional de recuperação.
Um publisher deve persistir estado antes da primeira operação Git mutável e produzir recibos imediatamente após cada marco. Se o processo falhar após `git commit`, o próximo passo é reconciliar commit, parent, tree, allowlist, blobs e remoto; nunca repetir o commit cegamente. A reconstrução do recibo precisa declarar que foi recuperado e manter a proveniência do erro original.

### D-5B3R-02 — Diagnósticos devem validar o contrato de ambiente do subprocesso

**Estado:** Accepted como regra operacional.
Scripts Python que leem `os.environ[...]` devem receber variáveis explicitamente exportadas ou argumentos explícitos. Preflight deve verificar variáveis obrigatórias e falhar com mensagem de contrato ausente, não lançar `KeyError` sem contexto. Fixtures devem testar variável ausente e presente.

### D-5C-01 — Não implementar Reactor sem consumidor real

**Estado:** Gate arquitetural obrigatório; consistente com ADR-0034 Proposed.
A branch `dc2bf33` contém projeção de contexto, ExecutionContext e Providers read-only, mas não contém um `RfqSourcingReactor` nem um call-site AMRI de Application/Delivery. `AtomicSourcingFlow` e o handler `finalize_rfq_sourcing` operam `FinalizeRfqSourcing` do ADR-0031; esse fluxo é uma capability composta existente, não o consumidor Reactor AMRI. O fluxo `docs/flows/atomic-rfq-sourcing-amri.md` declara o cenário-alvo como Proposed, not implemented. Não o tratar como requisito de produção já confirmado.

**Saída:** antes de código, demonstrar uma interação real que exija fatos de três capabilities independentes no mesmo commit atômico, identificar owners e call-site de Delivery/Application. Se não houver consumidor, publicar blocker report e não criar endpoint/facade/scaffolding.

### D-5C-02 — Resolver discrepância do DAG antes de integrar

A `interaction_definition()` em `src/amri_participants/rfq_sourcing.rs` declara Close e Award sem dependência entre si; Create PO depende de Award. O diagrama em `docs/flows/atomic-rfq-sourcing-amri.md` mostra uma aresta Close → Award, enquanto o texto do próprio documento pede validação pelo domain owner. O comportamento implementado/documentado na Stage 4B prevalece: Close e Award partem independentes e observam o mesmo snapshot até uma decisão explícita, baseada em regra de domínio, alterar esse DAG. Não inserir a aresta apenas para reproduzir o desenho.

### Estado global

ADR-0034 continua `Proposed`. Nenhum dos eventos de publicação documental aceita automaticamente as propostas AMRI-D01…D16.

<!-- AMRI-STAGE5B3R-DECISIONS-20261121:END -->
