# Guardrails de engenharia e automação — AMRI

**Aplicação:** todas as fases AMRI, scripts de manutenção, implementação, verificação e publicação.

Este documento consolida os aprendizados operacionais das entregas anteriores. Preservar o código, os contratos aceitos e o estado do Git tem precedência sobre automatizar uma mudança.

## 1. Ciclo obrigatório de change set

1. Ler o estado atual, plano da fase, ADRs relevantes e código real na branch.
2. Confirmar branch, HEAD local, HEAD remoto, origin e working tree.
3. Fazer análise de impacto e definir allowlist exata antes de gerar alterações.
4. Gerar diff completo automaticamente, antes de aplicar.
5. Executar validação estática e testes dos scripts em fixtures isoladas.
6. Criar e verificar backup antes de qualquer escrita no repositório.
7. Aplicar um único change set delimitado.
8. Executar gates apropriados e registrar cada código de saída.
9. Conferir todo o diff e o conjunto de arquivos modificados.
10. Atualizar evidências e documentação no mesmo change set.
11. Preparar commit e push em operações separadas, somente mediante autorização explícita.

## 2. Scripts fail-closed

- O modo padrão é `--dry-run`.
- Âncoras de substituição devem ocorrer exatamente a quantidade esperada; zero ou múltiplas ocorrências abortam antes de escrever.
- Confirmar branch, HEAD, remote HEAD e hashes dos arquivos-base.
- Recusar working tree suja quando o change set exigir baseline limpo.
- A allowlist deve ser explícita e verificada antes e depois da aplicação.
- Nunca usar `git add .`, `git reset --hard`, `git clean -fd` ou force push.
- Não sobrescrever arquivos divergentes nem corrigir automaticamente baseline desconhecido.
- Toda escrita exige backup verificável e procedimento de rollback.
- Commit, push e merge não fazem parte do `--apply`.

## 3. Heredocs e geração de conteúdo

- Usar delimitadores de heredoc entre aspas simples (`<<'EOF'`) para preservar Markdown, crases, `$`, barras e exemplos.
- Não usar `eval` nem command substitution sobre conteúdo documental.
- Não usar substituições genéricas sem verificar quantidade e contexto das âncoras.
- Testar conteúdo com backticks, `${...}`, `$()`, barras invertidas e blocos Markdown em fixtures.
- `bash -n` não substitui o teste funcional do gerador em fixture.

## 4. Artefatos e evidências

O próprio script deve gerar e conferir, sem montagem manual:

- diff integral e diff stat;
- logs stdout/stderr e códigos de saída;
- relatório por gate e resumo consolidado;
- manifesto de arquivos e tamanhos;
- backup e instrução de rollback;
- `SHA256SUMS` e checksum externo do pacote;
- pacote `.tar.gz`;
- recibo externo de verificação.

Ordem obrigatória: concluir arquivos, gerar manifesto, gerar checksums, empacotar e verificar o pacote. O recibo externo é criado depois do pacote para evitar hash autorreferencial.

Verificar `sha256sum -c`, `tar -tzf`, caminhos/tipos, extração temporária, checksums após extração e correspondência do manifesto. Arquivo vazio só é permitido quando previsto e justificado. Log vazio não comprova exit code zero.

## 5. Validação do próprio script

Antes da entrega: `bash -n`, `--help`, caminhos com espaços, fixture limpa, working tree suja, baseline divergente, rollback, arquivos gerados, pacote e diff.

Se uma etapa falhar, não imprimir resultado global `PASS`.

## 6. Testes Rust e isolamento

- Functional Core e AMRI Core: testes puros, property tests e mutação restrita aos módulos explicitamente inventariados.
- Prepared participants e Reactor/Providers: Fakes e testes unitários de library.
- PostgreSQL/Testcontainers: somente nos gates de adapter, concorrência e persistência que exigirem infraestrutura.
- `cargo check --all-targets` compila alvos; não significa que testes de integração foram executados.
- `cargo test --lib` com filtro não equivale a todos os targets.
- Não executar mutation testing de Application State, Providers, HTTP, adapter ou shell.
- Não introduzir Testcontainers em campanhas de mutação ou validação pura.
- Registrar testes pulados separadamente; nunca chamá-los de aprovados.

## 7. Evidência e comunicação de status

Usar `Proposed`, `Implemented`, `Verified`, `Accepted` e `Reported by maintainer` sem misturá-los. Toda evidência deve conter comando, exit code, resultado, ambiente, commit, data, logs e limitações. Não inventar contagens ausentes.

Aprovação local não significa CI aprovado. Publicação Git não prova compilação. `cargo check` não prova comportamento. Teste puro não prova atomicidade do banco.

## 8. Proteção das fases AMRI

- Stage 5 prepara e coordena; não persiste.
- Stage 6 valida composição e sela o plano de commit; não persiste.
- Stage 7 implementa commit atômico, marker e recovery.
- Não antecipar responsabilidades de fases posteriores.
- Preservar RPUs autônomos, `FinalizeRfqSourcing`, `crate::domain` como `pub(crate)` e contratos publicados.
- Não alterar dependências ou generalizar abstrações sem necessidade comprovada.
- Refinamentos arquiteturais devem atualizar ADR-0034 e decision register.

## 9. Publicação e continuidade

- Commit separado do push.
- Push somente fast-forward; nunca usar force para contornar divergência.
- Verificar SHA remoto após publicação.
- Atualizar `current-state.md`, `stage-plan.md`, `stage-log.md` e `quality-and-evidence.md` no mesmo change set documental.
- O handoff deve apontar artefatos reais, checksums, branch, HEAD, gates e pendências.

<!-- AMRI-STAGE5B3R-PUBLISHER-LEARNINGS:START -->

## Aprendizados adicionais de publishers — 2026-11-20

- Persistir estado antes de validação, gate ou publicação potencialmente
  falha; `status` deve funcionar mesmo após falha parcial.
- Não depender exclusivamente de `git diff --cached`: cruzar porcelain v2,
  índice (`git ls-files --stage`), nomes e blob hashes.
- Validar patch completo com `git apply --stat` e `git apply --check`
  contra o baseline exato; SHA-256 comprova identidade, não aplicabilidade.
- Não sobrescrever run directories, worktrees ou pacotes distribuídos.
- Preservar logs originais; sanitizar somente cópias publicáveis.
- `SKIPPED` nunca deve ser convertido em PASS.
- Push sem force, com verificação do SHA remoto antes e depois.

<!-- AMRI-STAGE5B3R-PUBLISHER-LEARNINGS:END -->


## 10. Isolamento de zsh e Bash

**Regra obrigatória:** comandos operacionais AMRI que usem opções estritas de
shell (`errexit`, `nounset`, `pipefail`) devem rodar em um processo Bash filho.
Nunca enviar um bloco colável que comece com `set -e`, `set -u`, `set -o
pipefail` ou combinações dessas opções para execução direta no nível superior
de uma sessão interativa zsh.

Padrão obrigatório para blocos coláveis no terminal do mantenedor:

```bash
bash <<'BASH'
set -Eeuo pipefail

REPO="/mnt/Dados/MeusDados/Downloads/supplier-rfq-rpu-rust"
# Demais comandos Bash do diagnóstico ou da operação ficam aqui.
printf 'REPO=%s\n' "$REPO"
BASH
```

Regras complementares:

1. O delimitador do heredoc deve ser literal e entre aspas simples (`<<'BASH'`),
   impedindo que o zsh expanda `$`, `${...}`, `$(...)`, crases e barras antes
   de o Bash receber o conteúdo.
2. Scripts distribuídos devem ter shebang Bash (`#!/usr/bin/env bash`) e ser
   executados como processo separado (`bash ./script.sh` ou `./script.sh`).
   Nunca usar `source script.sh` ou `. script.sh`.
3. `set -Eeuo pipefail` deve ficar dentro do arquivo Bash ou do heredoc filho,
   depois de validado o interpretador; nunca antes do comando `bash` no shell
   interativo.
4. Não usar `emulate`, `setopt`, `unsetopt` ou alterações no prompt do zsh como
   substituto para isolamento de processo. O material distribuído pelo AMRI
   deve continuar Bash explícito e independente da configuração pessoal do
   shell do mantenedor.
5. Se `nounset` tiver sido ativado acidentalmente na sessão zsh, recuperar a
   sessão com `unsetopt UNSET PIPE_FAIL`. Não presumir que isso identifica ou corrige um bug no
   tema/prompt, nem alterar arquivos de configuração para mascarar o incidente.
6. Antes da entrega, validar scripts com `bash -n`; testar a execução no Bash
   filho e, quando zsh estiver disponível, verificar que as opções do shell
   pai permanecem inalteradas após o heredoc terminar.

Esta regra é consequência direta do incidente de 2026-11-21 registrado em
[engineering-learnings.md](./engineering-learnings.md).

## 11. Runner executável obrigatório em toda entrega

**Regra invariável:** toda entrega operacional preparada pelo assistente deve
incluir um script executável que rode os artefatos entregues. O README, um
patch, uma sequência de comandos ou um trecho para copiar/colar não substitui
essa obrigação.

Requisitos mínimos:

1. Gerar um runner específico do pacote, com shebang Bash e modos claros
   (`--help`, `verify`, `diagnose`, `preflight`, `apply` ou o conjunto
   equivalente adequado ao artefato).
2. Se o runner estiver dentro de `.tar.gz` ou outro pacote que precise ser
   extraído antes de ser executado, entregar também um launcher externo que
   verifique o checksum, valide os caminhos do arquivo compactado, faça
   extração temporária segura, preserve diretórios/pacotes existentes e
   invoque o runner interno. Não exigir que o mantenedor escreva o launcher.
3. O modo padrão deve ser não destrutivo. Escrita/aplicação exige modo ou
   argumento explícito e deve repetir os prechecks imediatamente antes dela.
4. O runner deve produzir logs, registrar exit codes, imprimir repositório,
   branch e HEAD quando aplicável, validar a allowlist, criar backup verificável
   antes da escrita e documentar/implementar rollback.
5. Commit, push, merge, `git add` e outras publicações nunca são implícitos;
   seguem autorização e precheck separados dos acordos de publicação.
6. Testar o próprio runner (`bash -n`, `--help`, fixture de sucesso, fixtures
   de falha, pacote/checksums e caminhos com espaços) antes de disponibilizá-lo.
7. Fornecer links diretos para o runner, pacote, checksum e README, e um único
   comando de execução que funcione no terminal do mantenedor. Não transferir
   ao usuário a montagem manual do script ou a orquestração dos arquivos.

A entrega só pode ser descrita como completa quando o runner e seu launcher,
quando necessário, existirem e tiverem sido verificados. Se não for possível
produzi-los, declarar explicitamente o bloqueio e não apresentar o pacote como
pronto para execução.
