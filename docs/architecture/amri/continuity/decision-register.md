# Registro de decisões, escolhas e invariantes AMRI

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

### Stage 4B proposta

- Participantes: `CloseRfqParticipant`, `AwardSupplierQuoteParticipant`, `CreatePurchaseOrderParticipant`.
- Cada participante devolve `PreparedOutcome` + fato proposto tipado, sem persistência.
- Close e Award são independentes; Create PO depende de Award.
- `AwardedSupplierQuote` mantém juntos award fact + source quote validada e detalhes de item.
- Definição declarativa de interação não equivale a executor/Reacto​​r; execução pertence à Stage 5.
- A V1 decidiu expor API experimental pública via `pub mod amri_participants` e `pub mod domain`. Essa escolha deve ser revista como trade-off de semver/API no diff, não é requisito imutável do protocolo genérico.

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
