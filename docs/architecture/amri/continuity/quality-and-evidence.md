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

<!-- AMRI-STAGE5B2-QUALITY:START -->

## Stage 5B.2 — matriz de evidências e pendências (2026-11-17)

| Evidência | Comando/critério | Estado neste pacote |
|---|---|---|
| Formatação | `cargo fmt --all -- --check` | Pendente no checkout do mantenedor |
| Build | `cargo check --all-targets --all-features --locked` | Pendente |
| Fakes/contratos puros | `cargo test --lib --all-features --locked app::amri::providers::tests -- --test-threads=1` | Pendente; filtro restrito aos Fakes/contratos unitários, não requer Docker |
| Imagem WireMock-RS | `bash scripts/build-amri-wiremock-rs-image.sh "$REPO"` | Pendente; requer Docker |
| Contrato HTTP integrado | `cargo test --test external_provider_contracts --all-features --locked` | Pendente; usa Testcontainers e a imagem local |
| Clippy | `cargo clippy --all-targets --all-features --locked -- -D warnings` | Pendente |
| Fronteiras | verify RPU, Application State, provider boundaries, Testcontainers e Functional Core isolation | Pendente |
| Coverage | scripts de Functional Core e AMRI Core via nightly explícito | Pendente; não alteram o escopo de produção da 5B.2 |
| Mutation | campanha consolidada | SKIPPED por política; não iniciar em 5B.2 |
| GitHub Actions | workflow associado ao commit da 5B.2 | Pendente; não inferir aprovação a partir do Verify local |

**Critério de honestidade:** nenhum `PASS` será escrito em documentos de evidência sem log não vazio, comando registrado, exit code zero e hash verificado. O Verify deve registrar gates individualmente. CI remoto e Verify local permanecem evidências separadas.
<!-- AMRI-STAGE5B2-QUALITY:END -->

<!-- AMRI-STAGE5B3R-FINAL-EVIDENCE:START -->

## Stage 5B.2 — re-verificação final publicada (2026-11-20)

- Branch de desenvolvimento: `feat/amri-stage4-three-prepared-participants`.
- HEAD verificado: `5fc082aee7e869908444b1bd6ac40d0df5c1721d`.
- Branch de evidência: `evidence/stage5b2-final-reverify-20261120T211047693659491Z`.
- Commit de evidência: `511fa159ecdcdb4f7c81eddda7c47be5d1474f9f`.
- Resultado: 17/17 PASS, zero FAIL.
- Gates recém-executados: 11; resultados históricos reutilizados: 6.
- Logs históricos validados: 12, com tamanho e SHA-256.
- Divergência de cinco arquivos explicada pelo commit
  `72d44d8433b23841a9dfb0e875dcfac4accb0fcd`; 21 arquivos permaneceram
  byte-idênticos à evidência de Apply.
- Mutation: `SKIPPED_BY_STAGE5B2_POLICY`.
- Promoção do status da fase: `NOT_PERFORMED`.
- CI remoto: não confirmado por este recibo.

A evidência permite revisão humana de encerramento da 5B.2; não promove
automaticamente a fase nem aceita ADR-0034.

<!-- AMRI-STAGE5B3R-FINAL-EVIDENCE:END -->

<!-- AMRI-STAGE5B3R-QUALITY-20261121:START -->

## Stage 5B.2 — revisão da evidência final (2026-11-21)

Fonte: branch `evidence/stage5b2-final-reverify-20261120T211047693659491Z`, commit `511fa159ecdcdb4f7c81eddda7c47be5d1474f9f`, arquivo `reverification-receipt.json`.

| Item | Resultado revisado |
|---|---|
| Gates | 17/17 PASS; 0 FAIL |
| Execução fresca | 11 gates reexecutados no source HEAD `5fc082aee7e869908444b1bd6ac40d0df5c1721d` |
| Reuso histórico | 6 gates reutilizados depois da validação de 12 registros de log por tamanho/SHA-256 |
| Pós-Apply drift | 5 paths revisados e vinculados ao commit `72d44d8433b23841a9dfb0e875dcfac4accb0fcd` |
| Mutation | `SKIPPED_BY_STAGE5B2_POLICY` — não é PASS e não é falha do gate da subetapa |
| Promoção automática | `NOT_PERFORMED` no recibo original |
| Fechamento no recibo | `ELIGIBLE_FOR_MANUAL_REVIEW` |
| CI remoto | Não confirmado; não declarar aprovado ou falho |

### Stage 5B.3A — source audit evidence-only

- Auditoria v1.1.2 publicada em `docs/architecture/amri/evidence/stage-5b3/source-audit-v1.1.2-20261120T191650Z/`.
- Source HEAD da coleta: `3324f28fbcdc928475ce2d01690af86d80798918`; 92 arquivos Rust coletados; `AUDIT_RESULT=PASS` e `CHECKOUT_MUTATION=NONE`.
- Bundle SHA-256: `8c1e7293d3132dc91628889b1984a6e4534444719e41cee122de47b2daccac70`.
- A publicação é evidence-only; não representa patch, aprovação de implementação ou conclusão da Stage 5C.

**Revisão:** conteúdo do recibo, README, publication manifest, sanitization manifest e resultados frescos foi inspecionado. Não foi identificada falha nos 17 gates registrados. A evidência sustenta conclusão de verificação local da 5B.2 com as limitações acima; não prova CI remoto nem execução de mutation.

### Stage 5B.3-R — registro da publicação de guardrails (2026-11-21)

- Commit: `dc2bf33fbd6e95f0fdbaad77b3f8c644af794f93`.
- Escopo: somente `engineering-learnings.md` e `engineering-guardrails.md`; nenhum Rust/Cargo/workflow/ADR foi alterado.
- Resultado informado pelo mantenedor no terminal: `PUSH_PUBLISHED_AND_VERIFIED`, 11/11 recovered commit gates PASS; remote HEAD igual ao commit; `FORCE_PUSH=false`.
- Auditoria pós-Apply: `POSTAPPLY_AUDIT_PASS`, `ALL_GATES_PASS=true`, working tree exata de dois arquivos, backups e hashes aprovados.
- Consulta ao GitHub para status combinado e workflow runs associadas a PR: listas vazias. O wrapper de workflow runs cobre PR-triggered runs; portanto, CI remoto permanece `NOT_CONFIRMED`.

### Próximo change set — gates documentais

- `git diff --check`;
- `bash scripts/verify-documentation-current-state.sh`;
- `bash scripts/verify-mermaid-diagrams.sh`.

Não executar Cargo, Testcontainers ou mutation testing para esta atualização docs-only. Registrar exit code e log real do runner. A Stage 5C ainda não tem gates de implementação até a discovery gate confirmar um consumidor real.

<!-- AMRI-STAGE5B3R-QUALITY-20261121:END -->
