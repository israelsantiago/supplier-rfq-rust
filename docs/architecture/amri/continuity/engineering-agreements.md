# Diretrizes e acordos de engenharia e colaboração

Este arquivo registra os acordos operacionais que devem ser seguidos em todas as fases AMRI e durante a colaboração nos chats.

## 1. Fonte de verdade e investigação

- Sempre consultar a branch e os arquivos atuais antes de propor alterações, em vez de assumir que um patch antigo ainda corresponde ao código.
- Preferir o commit remoto verificado do GitHub como referência auditável; código local só é fonte confiável depois de conferir branch/HEAD/status.
- Ao usar memória de chats anteriores, diferenciar explicitamente decisão conversacional de implementação/evidência no repositório.
- Não inventar detalhes de execução que não constem em output, CI ou código.
- Não solicitar ao mantenedor que repita informação já registrada neste pacote ou que pode ser lida diretamente do repositório.

## 2. Branch, commits e publicação

- Branch de trabalho deste fluxo: `feat/amri-stage4-three-prepared-participants`.
- Uma fase por change set/commit revisável; não misturar produção de fase seguinte no commit da atual.
- O mantenedor adotou a regra: **a cada fase concluída, commit e push da mesma branch**, para que a próxima conversa possa revisar código publicado.
- `--dry-run`, `--commit` e `--push` devem ser operações distintas.
- Push somente fast-forward; nunca usar `--force`, `--force-with-lease`, reset destrutivo, rebase/merge automático ou apagar branch como conveniência.
- Verificar HEAD remoto depois do push e registrar SHA/URL.
- Usar `git add` com paths exatos; nunca `git add .` ou `git add -A` em scripts de publicação.
- O push não substitui revisão do diff nem evidência de testes.

## 3. Scripts seguros e reproduzíveis

Todo script de patch/aplicação/publicação deve:

1. Exigir o repo como argumento ou variável explicitamente validada.
2. Confirmar root Git, branch, HEAD esperado ou relação de ancestralidade, origin e working tree/index.
3. Falhar fechado se anchor/count/hash/allowlist divergir.
4. Ter `--dry-run` como modo padrão e não escrever no dry-run.
5. Gerar diff integral e SHA-256 verificável.
6. Criar backup antes da primeira escrita, com branch, HEAD base, timestamps, lista de paths e hashes.
7. Usar arquivos temporários no mesmo filesystem e rename atômico (`os.replace`) para substituir arquivo.
8. Tratar rollback e detectar estado parcial; nunca tentar “consertar” automaticamente divergência desconhecida.
9. Ser idempotente ou abortar com mensagem clara se detecta aplicação parcial.
10. Não fazer commit/push no script de aplicação.
11. Não fechar terminal/IDE: orientar execução via `bash script.sh`, sem `source` e sem `exec` para shell de usuário.
12. Exibir `PASS`, `FAIL`/`ABORT`, caminho de backup e próximo comando permitido.

Não usar `sed`/regex ampla como substituição frágil em código Rust. Preferir âncoras únicas verificadas, transformações determinísticas ou geradores que produzem arquivo completo e formatado.

## 4. Controle de escopo

- Toda fase mantém allowlist exata de arquivos.
- Nenhuma alteração paralela ou limpeza incidental é permitida na mesma aplicação.
- Não alterar APIs de RPUs autônomas, `FinalizeRfqSourcing`, HTTP ou migrations sem decisão explícita.
- Preservar APIs existentes, erros/rejeições e ordem de validação salvo decisão/ADR e testes que provem a mudança.
- Não introduzir abstração genérica prematuramente; preferir composição estática e tipada na primeira interação RFQ.
- Não promover `pub` por acidente: mudanças de visibilidade devem aparecer em design note, diff e decisão registrada.

## 5. Limites de domínio e efeitos

- Functional Core é puro: sem I/O, Provider, storage, clock ou random IDs.
- Providers são injetados na shell/coordenação; values necessários são capturados uma única vez por execução lógica e passados explicitamente.
- Prepared participant não persiste nem executa side effect; só retorna resultado tipado.
- Uma capability não importa módulos internos de outra capability nem chama seu shell.
- AMRI Core é genérico e não depende de Supply Chain/RFQ/FACTSTR/PostgreSQL/SQLx/Axum.
- O Reactor coordena, não decide regras de domínio; não recebe EventStore ou conexão SQL.
- Uma transação/append em um store não fornece atomicidade com serviço externo.

## 6. Proteção de consistência

- A unidade de consistência é o read set/contexto que fundamenta a decisão, não um aggregate escolhido de antemão.
- AMRI deve adquirir uma leitura coerente para a união de requirements.
- O token é opaco, vinculado à store e à especificação do snapshot, e é repassado inalterado.
- Nunca combinar tokens de contextos separados com `max`, timestamps ou soma.
- O adapter deve reconstruir a mesma query e executar apenas uma confirmação atômica do lote.
- Conflito significa que a decisão ficou obsoleta; abortar a interaction e re-preparar participantes dependentes com snapshot novo, sem reutilizar lote antigo.

## 7. Testes, Testcontainers e mutation

- Teste de regra pura recebe valores em memória e não inicia Postgres/Testcontainers.
- Testes de adapters, `append_if`, concorrência e semântica de storage podem usar PostgreSQL/Testcontainers em target separado.
- **Mutation testing é apenas para Functional Core, AMRI Core e composition policies puras expressamente selecionadas.** Nunca invocar Testcontainers, Docker ou infraestrutura através do mutation runner.
- Não ampliar escopo de mutação para shell, Provider, adapter, I/O ou teste de integração.
- A campanha consolidada de mutation foi adiada até a implementação AMRI completa; não relatar uma campanha não executada.
- Não enfraquecer guards ou remover testes para fazer o pipeline passar.

## 8. Toolchain e gates

- Rust Edition 2024, `Cargo.lock` travado nas validações reproduzíveis.
- Preservar `cargo fmt --all -- --check`, `cargo check --all-targets --all-features --locked` e `cargo clippy --all-targets --all-features --locked -- -D warnings`.
- Executar testes unitários com filtros explícitos para lógica pura, sem iniciar alvos de integração por acidente.
- Rodar `verify-rpu-boundaries.sh`, `verify-application-state-boundary.sh`, `verify-functional-core-test-isolation.sh`, `verify-openapi.sh`, `verify-mermaid-diagrams.sh` e `verify-documentation-current-state.sh` no escopo apropriado.
- Distinguir build/check/clippy dos testes executados. Clippy compilar testes, mas não executa Testcontainers.
- Em scripts que formatam arquivos temporários isolados, não depender de opções instáveis como `skip_children`. Formatar e verificar individualmente fontes sem módulos filhos externos, preservar a configuração local do rustfmt e validar estruturalmente alterações triviais de registro de módulos. Após aplicar, exigir `cargo fmt --all -- --check` no crate completo.

## 9. Comunicação de execução

- Antes de fase complexa, apresentar plano curto.
- Durante execução, enviar atualizações com resultados duráveis; não prometer trabalho em background.
- Ao pedir logs, solicitar saída completa dos comandos relevantes, especialmente exit code, branch, HEAD, `git status`, diff stats e CI.
- Não declarar sucesso sem output verificável. Dizer explicitamente quando testes foram pulados ou toolchain não estava disponível.
- Uma etapa com falha de gate não recebe commit/push e bloqueia a etapa dependente até correção.

## 10. Continuidade e janela de contexto

O mantenedor pediu que, **a cada nova execução no chat ou início de fase**, seja informado o status da janela de contexto.

Como não existe medidor confiável de percentual de tokens restantes disponível ao assistente, a comunicação deve ser qualitativa e honesta:

- `Contexto: suficiente` — histórico atual e documentos permitem seguir sem risco evidente.
- `Contexto: extenso` — sessão longa; usar este pacote e o commit remoto como fontes primárias.
- `Contexto: retomada recomendada` — antes de acumular novas tarefas, abrir novo chat e colar o conteúdo de `chat-handoff.md`.

Nunca inventar percentual/quantidade restante. Ao iniciar uma nova fase, primeiro consultar `current-state.md` e confirmar o HEAD local/remoto. Atualizar o status da janela no comentário ou resposta visível. Se a sessão for retomada, o `chat-handoff.md` prevalece sobre lembranças incompletas do chat.

## 11. Regra de saída de fase

Uma fase só termina quando código/documentação revisados, gates aplicáveis passam, diff/allowlist limpos, commit foi criado, push fast-forward confirmado e o estado/histórico Markdown foi atualizado. Se usuário optar por não publicar, registrar explicitamente como trabalho local não publicado.
