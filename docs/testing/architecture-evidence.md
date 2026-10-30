# Matriz de princípios arquiteturais e evidências verificáveis

Esta matriz separa **intenção arquitetural**, **teste executável**, **comando de reprodução** e **limites da evidência**. Um nome citado nesta tabela deve existir no código; uma CI verde prova apenas os gates que efetivamente executou.

## EventQuery / Command Context Consistency (CCC)

| Princípio / contrato | Evidência executável | Como reproduzir | O que a evidência demonstra |
|---|---|---|---|
| `EventQuery` combina filtros distintos por `OR` | `event_query_ors_separate_filters` em `tests/acceptance_api.rs` | `cargo test --test acceptance_api event_query_ors_separate_filters -- --nocapture` | Um filtro encontra `rfq_opened.v1` e outro encontra `supplier_registered.v1`; ambos aparecem no resultado. |
| Tipos e predicados dentro do `EventFilter` são alternativas (`OR`) | `event_filter_ors_event_types_and_payload_predicates` | `cargo test --test acceptance_api event_filter_ors_event_types_and_payload_predicates -- --nocapture` | O filtro encontra os dois tipos, cada um pelo predicado de payload correspondente. |
| Tipo do evento e condição de payload precisam corresponder conjuntamente (`AND`) | `event_filter_requires_event_type_and_payload_match` | `cargo test --test acceptance_api event_filter_requires_event_type_and_payload_match -- --nocapture` | Um payload correspondente em um tipo não permitido não satisfaz o filtro. |
| Unicidade natural de RFQ e fornecedor usa `ID OR referência` | `acceptance_rejects_duplicate_natural_references_without_appending_facts` | `cargo test --test acceptance_api acceptance_rejects_duplicate_natural_references_without_appending_facts -- --nocapture` | A referência duplicada é detectada mesmo com novo ID; o estado autoritativo continua com um fato por referência. |
| Decisão de cotação perde validade quando elegibilidade muda | `ccc_supplier_eligibility_change_invalidates_a_prepared_quote` em `src/capabilities/submit_supplier_quote/process_request/ccc_tests.rs` | `cargo test --lib ccc_supplier_eligibility_change_invalidates_a_prepared_quote -- --nocapture` | O append preparado contra um contexto obsoleto é recusado e não grava a cotação. |
| Append concorrente com a mesma versão de contexto não aceita duas cotações duplicadas | `ccc_two_independent_stores_from_same_snapshot_commit_exactly_one_quote_fact` em `src/capabilities/submit_supplier_quote/process_request/ccc_tests.rs` | `cargo test --lib ccc_two_independent_stores_from_same_snapshot_commit_exactly_one_quote_fact -- --nocapture` | Duas conexões/instâncias FACTSTR independentes usam a mesma versão esperada; somente um append condicional é confirmado. |
| Contextos de fornecedores independentes não geram conflito lógico falso | `ccc_independent_supplier_contexts_commit_without_false_conflict` em `src/capabilities/submit_supplier_quote/process_request/ccc_tests.rs` | `cargo test --lib ccc_independent_supplier_contexts_commit_without_false_conflict -- --nocapture` | Cotação para dois fornecedores distintos pode ser aceita sem invalidar a outra pelo escopo lógico do contexto. Isso não prova paralelismo físico no PostgreSQL. |
| `SetSupplierEligibility` é uma atribuição de estado, não transição condicionada ao estado anterior | `eligibility_history_does_not_change_assignment_context_version` em `src/capabilities/set_supplier_eligibility/load_context.rs` | `cargo test --lib eligibility_history_does_not_change_assignment_context_version -- --nocapture` | Confirma a decisão explícita de excluir o histórico de elegibilidade da versão do contexto dessa capability. Se a regra mudar para transição, este contrato e o teste devem mudar juntos. |

## Functional Core / Imperative Shell

| Princípio | Evidência executável | Como reproduzir | Limite da evidência |
|---|---|---|---|
| Regras determinísticas são testadas sem HTTP/DB | Testes unitários em `src/capabilities/submit_supplier_quote/decision.rs`, incluindo `calculates_quote_total_with_integer_minor_units`, rejeições de domínio e proptest `property_quote_total_equals_sum_of_integer_line_totals` | `cargo test --lib` | Os testes cobrem as regras e propriedades enumeradas, não todo estado de produção possível. |
| A cobertura de branches do core é um gate específico | `scripts/coverage-functional-core.sh` e `scripts/check-functional-core-branch-coverage.py` | `bash scripts/coverage-functional-core.sh` | A meta é de 100% somente para as regiões/condições explicitamente incluídas no checker; não significa 100% de todo o crate. |
| Limites entre capabilities/RPUs e HTTP são verificados estruturalmente | `scripts/verify-rpu-boundaries.sh` | `bash scripts/verify-rpu-boundaries.sh` | As verificações por regex/estrutura são guardrails; não substituem revisão de design nem testes funcionais. |

## Event Sourcing, rejeições e read model

| Princípio | Evidência executável | Como reproduzir | O que a evidência demonstra |
|---|---|---|---|
| Comando rejeitado não deve persistir cotação | `acceptance_rejects_blocked_supplier_without_appending_quote_fact` | `cargo test --test acceptance_api acceptance_rejects_blocked_supplier_without_appending_quote_fact -- --nocapture` | A resposta é uma rejeição de negócio e a consulta ao Event Store não encontra fato de cotação para o fornecedor. |
| Registro e elegibilidade são pré-condições para cotação | `acceptance_requires_registration_and_eligibility_before_quote_submission` | `cargo test --test acceptance_api acceptance_requires_registration_and_eligibility_before_quote_submission -- --nocapture` | Fornecedor ausente ou sem elegibilidade leva a rejeição e não gera fato de cotação. |
| Projeção é derivada e acompanha o ciclo de vida da RFQ | `acceptance_rfq_summary_projection_tracks_open_quote_close_and_paginated_reads` | `cargo test --test acceptance_api acceptance_rfq_summary_projection_tracks_open_quote_close_and_paginated_reads -- --nocapture` | Exercita atualização eventual do resumo após abertura, cotação e fechamento, além da consulta paginada. Não é um benchmark nem uma prova completa de rebuild/recuperação. |
| Contrato HTTP/OpenAPI permanece verificável | `dto_boundary_tests`, `tests/acceptance_api.rs` e `docs/openapi.yaml` | `cargo test --lib dto_boundary_tests`; `cargo test --test acceptance_api`; `bash scripts/verify-openapi.sh` | Exercita DTOs, comportamento HTTP aceito e lint do contrato publicado. |

## Gate completo antes do push

Execute `bash scripts/verify-rpu-boundaries.sh`, `bash scripts/coverage-functional-core.sh` e `bash scripts/verify-openapi.sh`. O primeiro executa `cargo fmt --check`, checks de compilação, todos os alvos de teste com `test-support`, Clippy com `-D warnings` e `git diff --check`. O segundo requer nightly/LLVM coverage e Docker conforme seus próprios pré-requisitos. O terceiro requer Node.js 22+ ou Docker.

O relatório gerado localmente pelo script de validação fica em `target/architecture-evidence/`; ele é evidência de execução da máquina/commit local, não deve ser confundido com o resultado do GitHub Actions. A evidência de CI deve ser verificada na página do commit/PR.

## Fonte externa da semântica de query

A semântica usada nesta matriz está implementada em [`factstr-postgres/src/query_match.rs`](https://github.com/ricofritzsche/factstr/blob/main/factstr-postgres/src/query_match.rs) no projeto FACTSTR. A suíte local acima é um contrato de regressão para o comportamento que as RPUs deste repositório dependem.

## ASP e estados/falhas da idempotência HTTP

| Contrato | Evidência executável | Como reproduzir | Limite |
|---|---|---|---|
| Capabilities não conhecem FACTSTR | `scripts/verify-application-state-boundary.sh` | `bash scripts/verify-application-state-boundary.sh` | Guardrail estrutural, complementado por revisão. |
| Adaptador lê fatos tipados e recusa append com versão obsoleta | `provider_reads_typed_facts_and_rejects_stale_conditional_append` | `cargo test --lib provider_reads_typed_facts_and_rejects_stale_conditional_append -- --nocapture` | PostgreSQL/Testcontainers; não é benchmark. |
| Um owner por chave e `unknown` não expira | `ledger_state_machine_fails_closed_and_expires_only_completed_records` | `cargo test --lib ledger_state_machine_fails_closed_and_expires_only_completed_records -- --nocapture` | Concorrência controlada, não stress de carga. |
| Ledger indisponível não executa comando | `http_idempotency_store_unavailable_prevents_command_execution` | `cargo test --test acceptance_api http_idempotency_store_unavailable_prevents_command_execution -- --nocapture` | Simula tabela indisponível em DB de teste. |
| Falha após append não duplica o fato | `http_idempotency_failure_after_append_marks_unknown_without_duplicate_fact` | `cargo test --test acceptance_api http_idempotency_failure_after_append_marks_unknown_without_duplicate_fact -- --nocapture` | Trigger SQL não equivale a matar o processo ou failover. |
| Requests concorrentes com a mesma chave não duplicam fato | `http_idempotency_concurrent_same_key_appends_one_fact` | `cargo test --test acceptance_api http_idempotency_concurrent_same_key_appends_one_fact -- --nocapture` | Duas requests controladas, não benchmark. |
| JSON semântico reordenado é reproduzido | `http_idempotency_replays_same_semantic_json_with_reordered_keys` | `cargo test --test acceptance_api http_idempotency_replays_same_semantic_json_with_reordered_keys -- --nocapture` | Ordena propriedades recursivamente; a ordem de arrays permanece significativa. |
| Resposta final de negócio 4xx é reproduzida | `http_idempotency_replays_final_business_4xx_response` | `cargo test --test acceptance_api http_idempotency_replays_final_business_4xx_response -- --nocapture` | 404 final preserva status/corpo e sinaliza replay. |
| Chave inválida/duplicada e request acima de 2 MiB não executam comando | `http_idempotency_rejects_invalid_key_and_oversized_request_before_command` | `cargo test --test acceptance_api http_idempotency_rejects_invalid_key_and_oversized_request_before_command -- --nocapture` | Limites específicos do middleware idempotente. |
| Path/query/Content-Type são parte do fingerprint | `http_idempotency_fingerprint_includes_path_query_and_content_type` | `cargo test --test acceptance_api http_idempotency_fingerprint_includes_path_query_and_content_type -- --nocapture` | Casos principais, não todos os headers possíveis. |
| Resposta acima de 4 MiB bloqueia replay incerto | `oversized_response_marks_key_unknown_and_blocks_retry` | `cargo test --lib oversized_response_marks_key_unknown_and_blocks_retry -- --nocapture` | Teste de middleware com router sintético e PostgreSQL, não falha real de streaming em produção. |

A cobertura 100% de Functional Core não mede automaticamente o ASP, fachadas, middleware, migrations ou cenários operacionais.

## AMRI — propriedades propostas, ainda sem implementação

**Importante:** a tabela abaixo é um plano de evidência, não uma afirmação de que os testes ou os módulos já existem. Os nomes são contratos-alvo a confirmar durante as etapas de implementação da ADR-0034. Até que o código exista e os comandos sejam executados, o estado é `Proposed / Not implemented`.

| Propriedade pretendida | Evidência automatizada-alvo | Camada / infraestrutura |
|---|---|---|
| Preparação não persiste | Testes de contrato por participante que verificam zero fatos depois de prepare, rejeição e erro | Participante + Fake State; aceitação com Testcontainers |
| Um snapshot cobre a união de requisitos | `planner_builds_one_canonical_composite_context` e `reactor_reads_snapshot_once_for_all_participants` | AMRI Core; Fake State |
| Não combinar tokens independentes | `composite_context_rejects_independent_snapshot_tokens` | AMRI Core; testes puros |
| Mesmo Functional Core nos dois modos | Testes de paridade entre execução autônoma e preparação com entradas equivalentes | Functional Core de cada capability |
| Grafo de participantes determinístico | Testes para ciclo, dependência ausente, participante duplicado e ordem estável | AMRI Core; testes puros/propriedade |
| Composição rejeita fatos incompatíveis | Matriz de políticas para RFQ, adjudicação e pedido | Composition Policies; testes puros/propriedade |
| Um único commit condicional | `amri_commits_all_participant_facts_and_marker_in_one_append_if` | FACTSTR/PostgreSQL Testcontainers |
| Conflito não persiste lote parcial | `amri_stale_composite_context_commits_zero_facts` | FACTSTR/PostgreSQL Testcontainers |
| Corrida entre interações | `amri_two_interactions_same_snapshot_commit_at_most_one_batch` | Stores independentes + Testcontainers |
| RPU autônoma concorrente respeita CCC | Testes de corrida entre `SubmitSupplierQuote`/`CloseRfq` e interação AMRI | Testcontainers |
| Idempotência e recuperação | Testes same-ID/same-fingerprint, same-ID/different-fingerprint e resposta perdida após commit | FACTSTR marker + fault injection |
| Core sem acoplamento | Build/test do Core sem RFQ, FACTSTR, PostgreSQL, SQLx e Axum | Cargo feature/dependency checks |
| Reutilização em outro domínio | Mesmo Core aplicado ao exemplo de reserva de recursos | Segundo domínio |

A matriz só deve promover uma propriedade para evidência implementada após apontar para o teste real, comando executável, resultado e limite da prova. Mutation testing AMRI deve permanecer restrito ao Core e às políticas puras; Testcontainers não pode ser acionado pelo runner de mutação do Functional Core.
