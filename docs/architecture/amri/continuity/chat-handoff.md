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
- Commit Stage 4A conhecido: `203589e4d2c23338c8040e7802fabc77203fd6a9`. O pacote de continuidade deve ser commitado/pushado depois, portanto este SHA não necessariamente será o HEAD atual. Consultar `git ls-remote` e atualizar current-state/stage-log com o SHA do commit documental.

**Situação conhecida:**

- Stages 0–3 concluídas conforme `docs/architecture/amri/implementation-plan.md` e evidências nele descritas.
- Stage 4A Shared Functional Core aplicada, verificada pelo mantenedor e publicada no commit `203589e4d2c23338c8040e7802fabc77203fd6a9`.
- Stage 4B ainda não foi aplicada. A V1 do patcher falhou no dry-run porque o rustfmt não formatou previamente os temporários e tentou resolver módulos filhos (`amri.rs`, `rfq_sourcing.rs`) ausentes do diretório temporário. A execução reportou explicitamente que nenhum arquivo do repositório foi alterado.
- Stage 4B V2 corrige o patcher sem usar opções instáveis de rustfmt: formata/verifica apenas `rfq_sourcing.rs` isoladamente, valida estruturalmente as adições triviais em `src/lib.rs` e `amri_participants/mod.rs`, preserva rustfmt config e exige `cargo fmt --all -- --check` após aplicação. Testes com mocks confirmam a ordem format-then-check.
- Objetivo da Stage 4B: `CloseRfqParticipant`, `AwardSupplierQuoteParticipant`, `CreatePurchaseOrderParticipant`, nenhum write, sem Reactor/Providers/append_if/marker/recovery.
- Grafo: Close e Award independentes; Create PO depende de Award.
- `AwardedSupplierQuote` é o hand-off selado e preserva award fact + quote original/linhas.
- Stage 4B prevê API pública experimental em `crate::amri_participants` e promoção de `crate::domain` para `pub`, devendo a superfície pública ser revisada explicitamente.

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

**Primeira tarefa da nova sessão:** confirmar o HEAD remoto depois do checkpoint documental. Executar/revisar os testes internos do patcher Stage 4B V2 e o dry-run V2 corrigido. Não aplicar até a revisão integral do diff. Se o pacote V2 não estiver disponível na nova conversa, reconstruí-lo a partir do estado descrito aqui e dos arquivos já presentes no ZIP anterior; não executar a V1 novamente.

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

## Atualização de continuidade — instalador documental V2

O `dry-run` documental V1 foi abortado por falso positivo ao comparar o append planejado com os documentos existentes `docs/architecture/amri/README.md` e `implementation-plan.md`. Nenhum arquivo foi alterado. Use exclusivamente `apply-amri-continuity-docs-v2.sh` e `publish-amri-continuity-docs-v2.sh` do pacote `amri-continuity-stage4b-v3.zip`. O instalador V2 preserva o texto preexistente, permite apenas os dois appends delimitados, é idempotente e recusa marker drift. Rode novamente os testes do instalador e `--dry-run`; não use os scripts V1.

## Primeira ação depois do pacote de continuidade V2

1. Usar `amri-continuity-stage4b-v3.zip` (não o V2 anterior) e rodar `test_amri_continuity_installer.py` e `test_stage4b_patcher_v2.py`.
2. Rodar `apply-amri-continuity-docs-v2.sh "$REPO" --dry-run`; revisar o append exato em `docs/architecture/amri/README.md` e `implementation-plan.md` e os novos arquivos sob `docs/architecture/amri/continuity/`.
3. Se passar, executar `--apply`; depois rodar `verify_amri_continuity_docs.py`, `verify-documentation-current-state.sh`, `verify-mermaid-diagrams.sh` e `git diff --check`.
4. Publicar documentação com publisher V2: `--dry-run`, `--commit` e `--push` separados. Confirmar SHA remoto.
5. Só então executar `test_stage4b_patcher_v2.py` e o dry-run real `apply-stage4b-prepared-participants-v2.sh`. Não aplicar Stage 4B até revisar todo o diff.
