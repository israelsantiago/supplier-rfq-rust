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
