# Handoff para iniciar um novo chat AMRI

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
