# Engineering learnings — AMRI

## 2026-11-20 — Stage 5B.2 final re-verification

- Recibo final: 17/17 gates PASS, zero FAIL.
- Onze gates foram executados novamente; seis históricos reutilizados
  após validação de integridade dos logs.
- Mutation foi SKIPPED pela política da subetapa, não PASS.
- Evidência-only publicada não equivale ao encerramento automático da fase.
- A ausência de status CI no recibo não permite declarar CI remoto aprovado.

## Publishers e automação

- Estado persistente deve ser criado antes dos gates e operações de publicação.
- `status` deve continuar funcional depois de uma falha parcial.
- `git diff --cached` isolado não é prova suficiente do índice; cruzar
  porcelain v2, `git ls-files --stage`, blobs e allowlist.
- O patch completo precisa passar `git apply --stat` e `git apply --check`
  no baseline exato. Hash não prova aplicabilidade.
- Pacotes são imutáveis e versionados; validar checksums internos e externos,
  inclusive após extração. Não incluir `__pycache__`.
- Logs originais não devem ser alterados; publicar cópias sanitizadas.
- Scripts são executados com `bash`, registram branch/HEAD e códigos de saída,
  preservam falhas e não modificam opções persistentes do shell.

## Arquitetura e sequência AMRI

- Stage 5B.3-R é documental; não criar abstrações sem consumidor.
- `FinalizeRfqSourcing` / `AtomicSourcingFlow` do ADR-0031 não é o Reactor AMRI.
- Close e Award observam o mesmo snapshot e são independentes.
- Create Purchase Order depende de `AwardedSupplierQuote`.
- Identity, fingerprint e timestamps permanecem estáveis em retries CCC;
  snapshot/token são renovados e resultados anteriores descartados.
- Providers são read-only; ausência de evidência e falha operacional são
  estados distintos. Não adicionar retry automático ou failover.
- Stage 5 exclui escrita, `append_if`, marker e recovery.
- ADR-0034 permanece Proposed até os gates posteriores.

## 2026-11-21 — isolamento de opções do shell (zsh/Bash)

- Incidente: um bloco de diagnóstico foi colado diretamente na sessão
  interativa zsh e executou `set -uo pipefail`. Embora os comandos tenham
  terminado, as opções alteradas permaneceram na sessão; ao retomar o prompt,
  o zsh reportou `parameter not set`.
- Causa operacional: `set` altera o shell corrente. Em uma sessão interativa,
  `nounset` (`-u`) também afeta a expansão feita posteriormente pelo prompt,
  por hooks (`precmd`/`preexec`) ou pelo tema. O erro observado depois do bloco
  não deve ser atribuído automaticamente ao último comando de diagnóstico.
- Correção imediata da sessão afetada: executar `unsetopt UNSET PIPE_FAIL` no zsh.
  Isso é recuperação pontual, não o mecanismo preventivo.
- Regra preventiva: todo bloco colável que configure `errexit`, `nounset` ou
  `pipefail` deve ser executado em processo filho Bash, usando `bash <<'BASH'`
  ou um arquivo `.sh` iniciado com `bash ./arquivo.sh`. Nunca executar essas
  opções no nível superior de uma sessão interativa zsh.
- Não executar scripts com `source` ou `.`; não alterar `~/.zshrc`, prompt ou
  tema para contornar um efeito colateral criado pelo próprio diagnóstico.
- A regra detalhada e o padrão executável estão em
  [engineering-guardrails.md, seção 10](./engineering-guardrails.md#10-isolamento-de-zsh-e-bash).

## 2026-11-21 — todo pacote entregue precisa de um runner executável

- Incidente de processo: o pacote corretivo v1.0.0 foi entregue com patch,
  README e comandos coláveis, mas sem um único script de entrada que
  orquestrasse a verificação, a instalação e a execução dos artefatos.
  Isso descumpriu o acordo de que o assistente sempre gera o script para
  executar os arquivos entregues.
- Regra: nenhum pacote operacional AMRI é considerado pronto para entrega sem
  um runner executável, específico daquele pacote, que automatize o fluxo
  documentado; exemplos de comandos no README não substituem o runner.
- Quando o runner estiver dentro de um arquivo compactado, entregar também
  um launcher externo que valide o checksum do arquivo, extraia com segurança
  sem sobrescrever pacotes anteriores e chame o runner interno.
- O runner deve oferecer modos explícitos e fail-closed (no mínimo `verify`,
  `preflight` e `apply`, quando aplicável), registrar logs/códigos de saída,
  criar backup verificável, conferir allowlist e oferecer rollback.
- Execução de aplicação deve ser uma ação explícita do mantenedor. O modo
  padrão é somente verificação; nunca executar commit, push ou merge.
- Validar `bash -n`, `--help`, execução em fixture, falhas esperadas, checksums,
  extração e caminhos com espaços antes da entrega. Informar com precisão
  quais verificações foram realmente executadas.
