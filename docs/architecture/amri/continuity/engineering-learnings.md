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

<!-- AMRI-PUBLISHER-LEARNINGS-20261121:START -->

## 2026-11-21 — receipts, publishers e diagnóstico de subprocessos

- Um `git commit` pode ter sido criado mesmo quando o publisher termina com erro antes de gerar seu recibo. Antes de repetir a operação, reconciliar `HEAD`, parent, subject, tree, allowlist, blobs, working tree e remote. Nunca repetir commit cegamente.
- Cada operação mutável deve persistir um estado/intention antes da operação e criar o recibo assim que o resultado for conhecido. O trap de falha deve preservar `FAILED_STEP`, linha, comando, exit code e log.
- O publisher deve validar os pós-estados independentemente do exit code do comando: push concluído e verificado é diferente de comando de push iniciado; commit criado e verificado é diferente de receipt criado.
- O diagnóstico de 2026-11-21 falhou com `KeyError: COMMIT_RECEIPT_VALID` porque a variável shell não foi exportada ao subprocesso Python. Variáveis consumidas por `os.environ` devem ser explicitamente exportadas, ou transmitidas por argumentos; validar contrato de ambiente antes de chamar o subprocesso.
- Fixtures de diagnóstico devem incluir variáveis ausentes, vazias e válidas. Erros internos do próprio diagnóstico não podem ser classificados como diagnóstico concluído.
- A recuperação bem-sucedida foi publicada em `dc2bf33fbd6e95f0fdbaad77b3f8c644af794f93`; execução informada: 11/11 recovered commit gates, push fast-forward e SHA remoto verificado.
- O runner `amri-stage5b3r-finalization-v1.0.0` falhou antes do Apply porque gerou `backup/SHA256SUMS` com `find . -type f` enquanto redirecionava a própria saída para esse mesmo arquivo. O shell criou/truncou `SHA256SUMS` antes de iniciar `find`; por isso o inventário capturou os dez documentos corretos e o próprio manifesto. A entrada autorreferente registrou o hash do arquivo vazio/inicial, que deixou de corresponder após a gravação do manifesto.
- A evidência `amri-stage5b3r-backup-failure-evidence-20261121T143116Z.tar.gz` confirmou 11 entradas, uma única divergência (`./SHA256SUMS`) e os dez documentos iguais aos blobs de baseline. Nenhum documento de origem estava ausente ou corrompido; o Apply não ocorreu.
- Manifestos de backup devem ser construídos somente a partir da allowlist explícita, em arquivo temporário fora do diretório inventariado; validar exatamente dez entradas, instalar por rename atômico e manter o checksum do manifesto em sidecar externo. Nunca descobrir arquivos de backup por `find` para montar o manifesto que reside no próprio diretório.
- O gate de backup deve comparar cada cópia ao blob Git de baseline, verificar a lista integral do tar e repetir a validação antes do commit, push e rollback. Fixtures devem rejeitar autorreferência, arquivo extra, ausente, alterado e arquivo compactado adulterado.
- A tentativa anterior também classificou incorretamente a ausência de `--confirm-apply` como `build-and-validate-patch`; cada confirmação explícita deve ter seu próprio `CURRENT_STEP` antes da validação da flag.

<!-- AMRI-PUBLISHER-LEARNINGS-20261121:END -->
