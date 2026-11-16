# Estratégia de qualidade, testes e evidências

## Checkpoint de publicação Stage 4B — 2026-11-03

- Commit publicado: `5d278958855e41277bd00573d2cf419a50ee875d`.
- A consulta GitHub para esse SHA retornou `total_count = 0` para statuses e `workflow_runs = 0` para Actions.
- Este fato comprova apenas a ausência de evidência CI consultável nesta revisão; não prova aprovação nem falha dos testes.
- Não foi anexado neste checkpoint log local de `cargo fmt`, `cargo check`, testes ou `cargo clippy`. Preencher a matriz abaixo somente com saídas efetivamente observadas pelo mantenedor.
- A atualização documental é docs-only; nenhum gate Cargo foi executado por este pacote.

## Evidência local Stage 4B — Audit V3 — 2026-11-04

**Proveniência:** saída do terminal fornecida pelo mantenedor. A auditoria reportou baseline e snapshot local aprovados.

| Gate | Resultado |
|---|---|
| cargo-fmt-check | PASS |
| cargo-check-all-targets | PASS |
| test-stage4b-prepared-participants | PASS |
| test-domain-functional-core | PASS |
| test-close-rfq-library | PASS |
| test-finalize-rfq-sourcing-decision | PASS |
| cargo-clippy-all-targets | PASS |
| structural-boundaries-and-test-isolation | PASS |
| application-state-boundary | PASS |
| functional-core-test-isolation | PASS |

- Total: 10 gates; falhas/bloqueios: 0.
- Baseline, integridade do handoff, snapshot local e checksums: PASS.
- Arquivos inesperadamente vazios: 0.
- Pacote V3 SHA-256: `3e50a879c1d3b1c911b73bcb54268911620f1066d755dbb55424b6ee627199d2`.
- Horário observado: `2026-11-04T14:10:31Z`–`2026-11-04T14:10:37Z`.
- O resumo do terminal não informa contagens individuais de testes nem versão Rust; esses valores permanecem não informados.
- Esta evidência representa validação local, não aprovação de CI remoto.

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

## Stage 5B audit e correção de CI — 2026-11-16

### Evidência de auditoria local V2

- Baseline: `921f60c4bb692cf8466e4a7e553c91dcf9bab923`.
- Toolchain local registrada: Cargo/Rust `1.99.1`; rustfmt `1.10.0-stable`.
- Seis gates da auditoria: 6 PASS, 0 FAIL.
- Testes reportados pelo pacote: 212 aprovados.
- Essa auditoria não equivalia à toolchain CI após o release Rust 1.100.0.

### Falha CI reportada pelo mantenedor

- Clippy `1.100.0` com `-D warnings` falhou em `src/capabilities/submit_supplier_quote/decision.rs:445` por `clippy::needless_range_loop`.
- A falha estava dentro do teste `rejects_sum_overflow_across_valid_line_totals`.
- Causa operacional: validação local em Clippy 1.99.1, enquanto o runner remoto resolveu a toolchain estável 1.100.0.

### Gates obrigatórios do change set 5B.1

- `rust-toolchain.toml` fixa `1.100.0`; CI estável usa `dtolnay/rust-toolchain@1.100.0`.
- `bash scripts/verify-rpu-boundaries.sh` — inclui `cargo fmt`, check all targets/features, test all targets/features e Clippy `-D warnings`.
- `bash scripts/verify-testcontainers-isolation.sh`.
- `bash scripts/verify-amri-core-boundaries.sh`.
- `bash scripts/coverage-functional-core.sh` e `bash scripts/coverage-amri-core.sh` em nightly explícito, como no workflow.
- `bash scripts/verify-openapi.sh`, `sh scripts/verify-mermaid-diagrams.sh` e `bash scripts/verify-documentation-current-state.sh`.
- Mutação fica fora do Verify padrão porque o workflow condiciona esse job a `workflow_dispatch`/`schedule`.

**Status:** todos os gates acima estão pendentes de execução após Apply. Não registrar PASS antes da saída observada.
