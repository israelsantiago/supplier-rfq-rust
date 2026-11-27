# Handoff para iniciar um novo chat AMRI

<!-- AMRI-HANDOFF-PROCUREMENT-COMMIT-20261125:START -->

## Checkpoint vigente — 2026-11-25

**Repositório:** `israelsantiago/supplier-rfq-rust`
**Branch:** `feat/amri-stage4-three-prepared-participants`
**HEAD baseline local/remoto:** `d720183fbe31ded306d4eb2b5ca8f948670e6f2b`
**Diretório local:** `/mnt/Dados/MeusDados/Downloads/supplier-rfq-rpu-rust`

### Estado de execução confirmado

- Stage 5C v1.0.3: Apply aprovado em 18 caminhos; `POST_APPLY_TEST=PASS`; `ALL_GATES_PASS=true`; `PRECHECK=PASS`; commit e push não executados.
- Recibos originais: `~/.../amri-stage5c-implementation-v1.0.3-runs/20261124T202417Z/{apply-receipt.json,precheck-receipt.json}`. O teste pós-Apply está referenciado em `LATEST_PASSING_TEST` no status do runner.
- A revisão humana do diff identificou R1 (checkpoints documentais desatualizados), R2 (nome semântico incorreto para fingerprint da especificação) e R3 (governança/política do contrato de orçamento ainda proposta), além de três cenários de projeção PO v2 sem teste específico.
- O change set corretivo v1.0.4 atualiza a documentação, renomeia o fingerprint, acrescenta os testes e formaliza o limite de orçamento v1. O seu Apply/Test/Precheck possui recibos próprios e não substitui os recibos v1.0.3.

### Próxima ação técnica

1. Executar o bootstrap v1.0.4 diretamente de `~/Downloads`; ele instala o pacote no diretório de dados, confere branch/HEAD/remoto/18 hashes do recibo original e executa `package-check`, `fixture-test` e `dry-run`.
2. Após `DRY_RUN=PASS`, revisar o patch corretivo e os logs.
3. Executar `apply --confirm-apply`, `test` e `precheck` do runner v1.0.4. Não repetir Apply v1.0.3.
4. Com os gates corretivos aprovados, Stage 6 pode iniciar como trabalho de validação estrutural/políticas/plano imutável, ainda sem storage write.

### Contrato Procurement Budget v1 — limite explícito

`procurement_budget_allocated.v1` e `procurement_budget_reserved.v1` são contratos propostos, não existentes no baseline d720. A alocação v1 é única e imutável por `budget_id`; reserva é positiva, na mesma moeda e limitada ao saldo; uma única reserva histórica por `(budget_id, rfq_id)`; não há evento de release, cancelamento com restituição ou ajuste em v1. Isso evita liberar saldo por suposição, mas torna a política imprópria para produção até o owner de negócio nomeado aprovar produtor, granularidade e ciclo de vida/release.

**Não inferir nem inventar o nome do owner:** o repositório não identifica uma pessoa/equipe responsável pelo domínio Financeiro/Procurement. A pendência está formalizada em `procurement-budget-contract.md`. Stage 6 pode trabalhar apenas sobre a proposta; Stage 7/produção permanecem bloqueados até aprovação formal e desenho de release/cancelamento.

### Limites preservados

- ADR-0034 permanece `Proposed`.
- Stage 5 não cria plano de commit nem escreve fatos.
- Stage 6 não executa escrita; Stage 7 continua sendo o primeiro estágio com `append_if`, marker e recovery.
- Não criar endpoint, shell Delivery, outbox ou integração ERP neste change set.
- Commit e push não são realizados pelos scripts e exigem autorização explícita separada.

<!-- AMRI-HANDOFF-PROCUREMENT-COMMIT-20261125:END -->


<!-- AMRI-CURRENT-CHAT-HANDOFF-20261121:START -->

## Checkpoint vigente — 2026-11-21

O prompt e o checklist históricos abaixo foram preservados para rastreabilidade; este checkpoint é a fonte vigente e substitui a antiga primeira tarefa de retomada.

**Branch:** `feat/amri-stage4-three-prepared-participants`
**HEAD publicado:** `dc2bf33fbd6e95f0fdbaad77b3f8c644af794f93`
**Commit:** `docs(amri): record shell isolation and executable runner guardrails`
**Push:** confirmado fast-forward; HEAD remoto igual ao local; sem force push.

### Estado e próximo trabalho

1. Stage 5B.2: recibo final da branch `evidence/stage5b2-final-reverify-20261120T211047693659491Z`, commit `511fa159ecdcdb4f7c81eddda7c47be5d1474f9f`: 17/17 PASS; 11 gates frescos; 6 históricos reutilizados depois de 12 logs validados; mutation SKIPPED por política; CI remoto não confirmado; fechamento no recibo elegível para revisão manual.
2. Stage 5B.3-R: publicação dos guardrails concluída; a reconciliação dos demais documentos e o handoff 5C estão neste change set. Não considerar a subetapa encerrada antes da publicação deste change set.
3. Stage 5C: não iniciar implementação de Reactor sem consumidor real na Application/Delivery. A branch atual contém context projection, ExecutionContext e read-only Providers, mas não contém Reactor nem call-site AMRI real.
4. `AtomicSourcingFlow`/HTTP `finalize_rfq_sourcing` executa `FinalizeRfqSourcing` do ADR-0031; não é Reactor AMRI e não deve ser adaptado artificialmente.
5. Resolver discrepância do DAG: Stage 4B declara Close e Award independentes; Create PO depende de Award. Um diagrama proposto em `docs/flows/atomic-rfq-sourcing-amri.md` mostra Close → Award. Domain owner deve decidir com base em regra real antes de alterar o grafo.
6. ADR-0034 permanece Proposed. Stage 5 não escreve; Stage 6 é composição/plano; Stage 7 é append_if único/marker/recovery.

### Ordem de leitura

- `current-state.md`
- `stage-plan.md`
- `decision-register.md`
- `engineering-agreements.md` e `engineering-guardrails.md`
- `quality-and-evidence.md` e `stage-log.md`
- `stages/stage-05-reactor-and-providers.md`
- `stages/stage-05c-handoff.md`
- `docs/architecture/adr/0034-atomic-multi-rpu-interaction.md`
- `docs/flows/atomic-rfq-sourcing-amri.md`

### Regras para a retomada

- Não repetir 17 gates 5B.2 sem mudança material.
- Não promover mutation SKIPPED nem inferir CI PASS.
- Confirmar o HEAD local/remoto antes de cada change set.
- Todo pacote operacional deve incluir o runner executável; heredocs Bash sempre `<<'BASH'` e execução em processo Bash filho.
- Commits e pushes separados, recibos estruturados, allowlist exata, patch completo validado, backups e rollback.
- Se a discovery não localizar consumidor real, gerar blocker report; não inventar API, facade, endpoint ou código morto.

**Primeira tarefa da próxima sessão:** concluir a publicação desta reconciliação 5B.3-R; depois executar apenas a discovery gate descrita em `stages/stage-05c-handoff.md`.

<!-- AMRI-CURRENT-CHAT-HANDOFF-20261121:END -->


Este arquivo foi escrito para abrir uma nova conversa sem depender do histórico anterior. Cole o texto entre as linhas no início do novo chat, junto com o ZIP/branch atual se necessário.

---

## Prompt de retomada

Estamos continuando o projeto Rust `supplier-rfq-rust`, uma referência de Supply Chain/RFQ baseada em Domain Capabilities, RPU, Functional Core / Imperative Shell, FACTSTR/CCC e uma extensão arquitetural proposta chamada AMRI (Atomic Multi-RPU Interaction).

**Leia primeiro os documentos abaixo na branch atual antes de propor alterações:**

- `docs/architecture/amri/continuity/README.md`
- `docs/architecture/amri/continuity/current-state.md`
- `docs/architecture/amri/continuity/architecture-and-boundaries.md`
- `docs/architecture/amri/continuity/decision-register.md`
- `docs/architecture/amri/continuity/stage-plan.md`
- `docs/architecture/amri/continuity/engineering-agreements.md`
- `docs/architecture/amri/continuity/quality-and-evidence.md`
- `docs/architecture/amri/continuity/stage-log.md`
- `docs/architecture/amri/implementation-plan.md`
- `docs/architecture/adr/0034-atomic-multi-rpu-interaction.md`

**Repositório e branch:**

- GitHub: `https://github.com/israelsantiago/supplier-rfq-rust`
- Branch: `feat/amri-stage4-three-prepared-participants`
- Diretório usual no Ubuntu: `/mnt/Dados/MeusDados/Downloads/supplier-rfq-rpu-rust`
- Commit Stage 4A: `203589e4d2c23338c8040e7802fabc77203fd6a9`. Commit de código Stage 4B publicado: `5d278958855e41277bd00573d2cf419a50ee875d`. O checkpoint documental desta sessão será publicado em commit separado; consultar `git ls-remote` para confirmar o SHA final.

**Situação conhecida no checkpoint de 2026-11-03:**

- Stages 0–3 e Stage 4A possuem implementação/documentação conforme os commits e evidências descritos no plano.
- Stage 4B foi publicada na branch `feat/amri-stage4-three-prepared-participants`, commit `5d278958855e41277bd00573d2cf419a50ee875d` (`feat(amri): add prepared RFQ sourcing participants (Stage 4B)`).
- O código inclui `CloseRfqParticipant`, `AwardSupplierQuoteParticipant` e `CreatePurchaseOrderParticipant`. Close e Award são independentes; Create PO depende do hand-off selado `AwardedSupplierQuote`.
- Os participantes preparam outputs/fatos em memória. Não implementam Reactor, Providers, policies, commit plan, `append_if`, marker, recovery ou retry.
- O módulo `crate::amri_participants` é experimental e seletivo; `crate::domain` permanece `pub(crate)` conforme o contrato documentado.
- ADR-0034 continua **Proposed**. A implementação Stage 4B não significa AMRI completo ou aceito.
- A consulta GitHub para o commit `5d278...` não encontrou status de CI nem workflow runs. A validação local de Rust/Cargo/Clippy precisa ser confirmada pelos logs do mantenedor; não presumir PASS.
- Próxima etapa planejada: Stage 5 — Reactor e Providers read-only, limitada a coordenação, snapshot único, dependências e abort. Stage 6 mantém composição; Stage 7 mantém o primeiro commit atômico.

**Regras obrigatórias:**

1. Antes de cada nova fase/execução, informe o status qualitativo da janela de contexto. Não invente percentual: não há medidor confiável de tokens restantes.
2. Consulte branch/HEAD/remote/status e código real antes de construir patch. Não reutilize patch antigo sem validar âncoras e hashes.
3. Uma fase por commit, e cada fase deve ser commitada e enviada à mesma branch após validação; push fast-forward sem force. Commit e push em passos separados.
4. Scripts devem ser fail-closed, idempotentes, backup antes de escrever, dry-run por padrão, allowlist exata, diff completo e SHA-256. Nenhum `git add .`.
5. Não fazer perguntas redundantes quando a informação está nos documentos ou pode ser consultada no GitHub.
6. Testes de mutation somente para Functional Core/AMRI Core/policies puras. Nunca usar Testcontainers/Postgres no mutation runner.
7. Preservar RPU autônomo, `FinalizeRfqSourcing` e a regra de ClockProvider/IdProvider; IDs/timestamps estáveis por execução lógica.
8. Não implementar etapas futuras antecipadamente: Stage 5 Reactor/Providers; Stage 6 policies/commit plan; Stage 7 único append_if/marker/recovery.
9. Distinguir fatos de propostas e execução informada pelo mantenedor de evidência realmente vista.
10. Não declarar AMRI completo/ADR-0034 Accepted até todos os gates Stage 7–9 e critérios finais serem comprovados.

**Primeira tarefa da nova sessão:** confirmar branch/HEAD/working tree local e remoto; recuperar os logs reais da validação Stage 4B ou executar novamente os gates locais documentados. Depois, preparar a Stage 5 somente contra o código Stage 4B publicado. Não antecipar composition policies (Stage 6) nem commit/marker/recovery (Stage 7).

---

## Checklist manual de retomada

```bash
REPO="/mnt/Dados/MeusDados/Downloads/supplier-rfq-rpu-rust"
BRANCH="feat/amri-stage4-three-prepared-participants"

git -C "$REPO" status --short
git -C "$REPO" branch --show-current
git -C "$REPO" rev-parse HEAD
git -C "$REPO" ls-remote --heads origin "refs/heads/$BRANCH"
```

Local e remoto devem coincidir e o working tree precisa estar limpo para iniciar novo patch.

## Histórico — instalador documental V2 antes da publicação Stage 4B

O instalador V2 e o pacote V3 referidos nesta seção pertencem ao fluxo histórico de continuidade anterior à publicação do código Stage 4B. Não são pré-requisitos operacionais atuais e seus comandos não devem ser executados novamente. O checkpoint atual é gerado pelo pacote V4.4.

## Histórico — primeiras ações planejadas antes da publicação Stage 4B

As cinco ações listadas nesta seção foram substituídas pelo checkpoint publicado em `5d278958855e41277bd00573d2cf419a50ee875d`. Consultar `current-state.md` e `stage-plan.md` para a sequência vigente; não executar comandos dos pacotes V2/V3 antigos.
