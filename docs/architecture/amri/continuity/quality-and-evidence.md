# Estratégia de qualidade, testes e evidências

## Checkpoint de publicação Stage 4B — 2026-11-03

- Commit publicado: `5d278958855e41277bd00573d2cf419a50ee875d`.
- A consulta GitHub para esse SHA retornou `total_count = 0` para statuses e `workflow_runs = 0` para Actions.
- Este fato comprova apenas a ausência de evidência CI consultável nesta revisão; não prova aprovação nem falha dos testes.
- Não foi anexado neste checkpoint log local de `cargo fmt`, `cargo check`, testes ou `cargo clippy`. Preencher a matriz abaixo somente com saídas efetivamente observadas pelo mantenedor.
- A atualização documental é docs-only; nenhum gate Cargo foi executado por este pacote.

## 1. Princípio

A evidência deve corresponder à propriedade que se afirma. `cargo check` não prova comportamento; teste puro não prova o adapter PostgreSQL; um teste de adapter não prova interoperabilidade externa; compilação de Stage 4B não prova atomicidade AMRI completa.

Toda linha da matriz de evidências deve registrar: propriedade, comando, saída/contagem, ambiente, commit, execução local/CI, skipped e responsável/data.

## 2. Pirâmide de testes

| Camada | O que testa | Dependência permitida |
|---|---|---|
| Functional Core / AMRI Core | Decisão, identidade, DAG, validação estrutural, policies puras | Valores em memória, property tests |
| Prepared participants | Paridade com decisões compartilhadas, hand-off tipado, erros, metadata e ausência de write port | Snapshot/projeção em memória |
| Reactor/Providers | Ordem, dependências, abort, timeout, evidência, cancelamento | Fake providers e fake state port |
| State port/adapter | Query, token, append condicional, contrato de codec | Adapter contract tests; PostgreSQL quando necessário |
| Acceptance/Concurrency | All-or-none, races, stale token, duplicate interaction, recovery | PostgreSQL/Testcontainers e fault injection |
| Operacional | Latência, throughput, contention, tracing e reconciliação | Ambiente de benchmark controlado |

## 3. Gating específico da Stage 4B

Depois de aplicar o patch V2, o verificador deverá fazer:

```bash
cargo fmt --all -- --check
cargo check --all-targets --all-features --locked
cargo test --lib --all-features --locked amri_participants::rfq_sourcing::tests -- --test-threads=1
cargo test --lib --all-features --locked domain:: -- --test-threads=1
cargo test --lib --all-features --locked capabilities::close_rfq:: -- --test-threads=1
cargo test --lib --all-features --locked capabilities::finalize_rfq_sourcing::decision::tests:: -- --test-threads=1
cargo clippy --all-targets --all-features --locked -- -D warnings
bash scripts/verify-rpu-boundaries.sh
bash scripts/verify-application-state-boundary.sh
bash scripts/verify-functional-core-test-isolation.sh
git diff --check
```

Os filtros devem confirmar que pelo menos um teste foi executado cada vez. O runner de Stage 4B deve evitar que a chamada ao boundary verifier repita `cargo test --all-targets` ou inicie Testcontainers. Usar a flag `SKIP_RUST_VALIDATION=1` só quando o script existente oferece esse contrato e as validações Cargo já foram executadas separadamente.

A Stage 4B não executa:

- `cargo test --all-targets` nem targets de integração;
- Testcontainers/PostgreSQL;
- coverage/mutation campaigns;
- benchmark;
- push/commit automático.

## 4. Mutation testing — regra invariável

A mutação é elegível somente para funções de lógica pura explicitamente inventariadas: Functional Core, AMRI Core e policies puras. O runner deve ter filtros que excluam testes PostgreSQL/Testcontainers, definir `DOCKER_HOST` inválido como defesa adicional e jamais executar/validar Docker. Não usar `cargo-mutants` no `src/application_state`, HTTP, Provider, adapter ou shell.

A campanha integrada será realizada após as fases centrais AMRI, e os resultados devem ser relatados por módulo/função, sem chamar cobertura do crate inteiro de cobertura do Functional Core.

## 5. Evidência por fase

| Fase | Evidência mínima | Infra |
|---|---|---|
| 0 | Baseline reproduzível, inventory e CI | Git/CI |
| 1 | ADR, protocolo, Mermaid/link check | Docs tooling |
| 2 | Unit/property, standalone boundary, branch/region coverage, mutation scoped | Rust puro |
| 3 | Planner/fake, adapter contract, token stale, OR/AND semântica FACTSTR e race | Fake + PostgreSQL Testcontainers |
| 4A | Paridade das funções puras, regressão RPU legacy, fmt/check/Clippy | Rust library puro |
| 4B | Três participantes, `AwardedSupplierQuote`, metadados DAG, fronteira sem I/O, Clippy | Rust library puro |
| 5 | Ordem topológica, abort, providers falhos/stale/timeout, stable identity/time, zero writes | Fakes |
| 6 | Matriz de combinação; invalid plan não é selado | Unit/property |
| 7 | Uma chamada append, zero parcial, marker, idempotência/recovery, corrida independente | PostgreSQL Testcontainers |
| 8 | Fault injection, load, métricas p50/p95/p99, runbook | Ambiente controlado |
| 9 | Segundo domínio, boundary compile/test sem RFQ/store | Rust puro + conformance |

## 6. Matriz de garantias a comprovar em Stage 7

- Exatamente uma chamada ao port/adapter de commit por tentativa confirmável.
- Um único `append_if` inclui todos os fatos de negócio e interaction marker.
- Rejeição de participante, policy, evidência externa, token stale ou falha antes de commit resulta em zero fatos daquele batch.
- Dois clientes com mesmo snapshot não conseguem ambos confirmar a mesma interação.
- Mesmo `InteractionId` + fingerprint igual retorna receipt já confirmado; mesmo ID + fingerprint diferente rejeita.
- Resposta perdida após commit permite reconciliação sem segundo append.
- Mudança em fato relevante invalida token; mudança fora do contexto não causa conflito falso quando contrato FACTSTR permitir.
- RPU autônomo concorrente também invalida o contexto AMRI se alterar fato protegido.
- Marker técnico não deve ser contado como fato de negócio nem contaminar projeções comuns.

## 7. Efeito de compilação/Clippy versus execução

`cargo check` e `cargo clippy` compilam os alvos solicitados, mas não executam os testes. `cargo test --lib` executa testes unitários da library conforme filtro. `cargo test --all-targets` pode incluir os alvos de integração e Testcontainers; não o usar inadvertidamente na campanha pura.

`cargo fmt --all -- --check` verifica o workspace completo; antes da aplicação, o patcher formata e verifica somente `rfq_sourcing.rs` como fonte isolada. As adições triviais ao crate root e ao módulo pai são verificadas estruturalmente; a verificação final do repo precisa ocorrer depois da aplicação. Não depender de `skip_children`, opção instável do rustfmt.

## 8. Registro de evidências

Para cada fase, acrescente ao `stage-log.md`:

```text
Commit base:
Commit final:
Branch:
Ambiente / Rust version:
Comandos executados (com exit code):
Testes executados / quantidade:
Testes pulados e razão:
CI URL / jobs / conclusão:
Diff stat / arquivos:
SHA-256 de pacote/patch:
Backups:
Limitações / riscos pendentes:
Revisor / data:
```
