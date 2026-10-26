# Matriz de princípios arquiteturais e evidências verificáveis

Esta matriz separa **intenção arquitetural**, **teste executável**, **comando de reprodução** e **limites da evidência**. Um nome citado nesta tabela deve existir no código; uma CI verde prova apenas os gates que efetivamente executou.

## EventQuery / Command Context Consistency (CCC)

| Princípio / contrato | Evidência executável | Como reproduzir | O que a evidência demonstra |
|---|---|---|---|
| `EventQuery` combina filtros distintos por `OR` | `event_query_ors_separate_filters` em `tests/acceptance_api.rs` | `cargo test --features test-support --test acceptance_api event_query_ors_separate_filters -- --nocapture` | Um filtro encontra `rfq_opened.v1` e outro encontra `supplier_registered.v1`; ambos aparecem no resultado. |
| Tipos e predicados dentro do `EventFilter` são alternativas (`OR`) | `event_filter_ors_event_types_and_payload_predicates` | `cargo test --features test-support --test acceptance_api event_filter_ors_event_types_and_payload_predicates -- --nocapture` | O filtro encontra os dois tipos, cada um pelo predicado de payload correspondente. |
| Tipo do evento e condição de payload precisam corresponder conjuntamente (`AND`) | `event_filter_requires_event_type_and_payload_match` | `cargo test --features test-support --test acceptance_api event_filter_requires_event_type_and_payload_match -- --nocapture` | Um payload correspondente em um tipo não permitido não satisfaz o filtro. |
| Unicidade natural de RFQ e fornecedor usa `ID OR referência` | `acceptance_rejects_duplicate_natural_references_without_appending_facts` | `cargo test --features test-support --test acceptance_api acceptance_rejects_duplicate_natural_references_without_appending_facts -- --nocapture` | A referência duplicada é detectada mesmo com novo ID; o estado autoritativo continua com um fato por referência. |
| Decisão de cotação perde validade quando elegibilidade muda | `ccc_supplier_eligibility_change_invalidates_a_prepared_quote` | `cargo test --features test-support --test acceptance_api ccc_supplier_eligibility_change_invalidates_a_prepared_quote -- --nocapture` | O append preparado contra um contexto obsoleto é recusado e não grava a cotação. |
| Append concorrente com a mesma versão de contexto não aceita duas cotações duplicadas | `ccc_two_independent_stores_from_same_snapshot_commit_exactly_one_quote_fact` | `cargo test --features test-support --test acceptance_api ccc_two_independent_stores_from_same_snapshot_commit_exactly_one_quote_fact -- --nocapture` | Duas conexões/instâncias FACTSTR independentes usam a mesma versão esperada; somente um append condicional é confirmado. |
| Contextos de fornecedores independentes não geram conflito lógico falso | `ccc_independent_supplier_contexts_commit_without_false_conflict` | `cargo test --features test-support --test acceptance_api ccc_independent_supplier_contexts_commit_without_false_conflict -- --nocapture` | Cotação para dois fornecedores distintos pode ser aceita sem invalidar a outra pelo escopo lógico do contexto. Isso não prova paralelismo físico no PostgreSQL. |
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
| Comando rejeitado não deve persistir cotação | `acceptance_rejects_blocked_supplier_without_appending_quote_fact` | `cargo test --features test-support --test acceptance_api acceptance_rejects_blocked_supplier_without_appending_quote_fact -- --nocapture` | A resposta é uma rejeição de negócio e a consulta ao Event Store não encontra fato de cotação para o fornecedor. |
| Registro e elegibilidade são pré-condições para cotação | `acceptance_requires_registration_and_eligibility_before_quote_submission` | `cargo test --features test-support --test acceptance_api acceptance_requires_registration_and_eligibility_before_quote_submission -- --nocapture` | Fornecedor ausente ou sem elegibilidade leva a rejeição e não gera fato de cotação. |
| Projeção é derivada e acompanha o ciclo de vida da RFQ | `acceptance_rfq_summary_projection_tracks_open_quote_close_and_paginated_reads` | `cargo test --features test-support --test acceptance_api acceptance_rfq_summary_projection_tracks_open_quote_close_and_paginated_reads -- --nocapture` | Exercita atualização eventual do resumo após abertura, cotação e fechamento, além da consulta paginada. Não é um benchmark nem uma prova completa de rebuild/recuperação. |
| Contrato HTTP/OpenAPI permanece verificável | `dto_boundary_tests`, `tests/acceptance_api.rs` e `docs/openapi.yaml` | `cargo test --lib dto_boundary_tests`; `cargo test --features test-support --test acceptance_api`; `bash scripts/verify-openapi.sh` | Exercita DTOs, comportamento HTTP aceito e lint do contrato publicado. |

## Gate completo antes do push

Execute `bash scripts/verify-rpu-boundaries.sh`, `bash scripts/coverage-functional-core.sh` e `bash scripts/verify-openapi.sh`. O primeiro executa `cargo fmt --check`, checks de compilação, todos os alvos de teste com `test-support`, Clippy com `-D warnings` e `git diff --check`. O segundo requer nightly/LLVM coverage e Docker conforme seus próprios pré-requisitos. O terceiro requer Node.js 22+ ou Docker.

O relatório gerado localmente pelo script de validação fica em `target/architecture-evidence/`; ele é evidência de execução da máquina/commit local, não deve ser confundido com o resultado do GitHub Actions. A evidência de CI deve ser verificada na página do commit/PR.

## Fonte externa da semântica de query

A semântica usada nesta matriz está implementada em [`factstr-postgres/src/query_match.rs`](https://github.com/ricofritzsche/factstr/blob/main/factstr-postgres/src/query_match.rs) no projeto FACTSTR. A suíte local acima é um contrato de regressão para o comportamento que as RPUs deste repositório dependem.
