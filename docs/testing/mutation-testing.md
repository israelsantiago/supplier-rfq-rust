# Mutation testing do Functional Core

A cobertura de ramos comprova que as alternativas foram exercitadas; ela não prova, sozinha, que as assertions detectam uma decisão errada. `cargo-mutants` complementa os testes alterando temporariamente o comportamento de funções e verificando se a suíte detecta a mudança.

## Executar

```bash
bash scripts/mutate-functional-core.sh
```

Se necessário, o runner instala `cargo-mutants` automaticamente na primeira execução; é necessário acesso à rede para baixar a ferramenta. O runner foca em `generate_consequences.rs`, `build_context.rs` e `submit_supplier_quote/decision.rs`, executando os testes unitários da library. O runner de mutation testing exclui explicitamente os três testes de integração de storage que iniciam PostgreSQL (`provider_reads_typed_facts_and_rejects_stale_conditional_append`, `replaying_the_same_fact_batch_does_not_double_count_quotes` e `failed_projection_batch_rolls_back_earlier_events`). O restante dos testes unitários da library e os testes puros do Functional Core continuam executando; um `DOCKER_HOST` inválido faz a etapa falhar se uma nova dependência acidental de Docker for introduzida. Esses testes de storage continuam obrigatórios na suíte completa/aceitação, que exige Docker. Para ajustar duração e paralelismo, use `MUTANTS_TIMEOUT=120 MUTANTS_JOBS=4`.

Os artefatos de `mutants.out/` são locais e ficam ignorados pelo Git. Priorize mutantes `MISSED`/`NOT CAUGHT`: leia a alteração gerada, determine se ela é semanticamente relevante e acrescente uma assertion objetiva. Não trate mutantes não compiláveis como cobertura obtida, nem exija que mutações semanticamente equivalentes sejam mortas. Mutation testing pode levar vários minutos; não é executado no CI rápido padrão.

Referência oficial: [cargo-mutants](https://mutants.rs/), [filtragem por arquivo](https://mutants.rs/skip_files.html), [interpretação de resultados](https://mutants.rs/using-results.html).


## Isolamento de Testcontainers

O mutation testing do Functional Core filtra exatamente nove testes de storage da library: o teste do adaptador FACTSTR, o teste de histórico de elegibilidade, os três cenários CCC de submissão de cotação, os dois testes do ledger HTTP e os dois testes da projeção. Eles continuam obrigatórios na suíte integral de regressão; são excluídos apenas do runner de mutações puras. `DOCKER_HOST` aponta para um socket deliberadamente inexistente como fail-fast contra regressões. O alvo puro `src/capabilities/finalize_rfq_sourcing/decision.rs` está incluído no escopo.


## Paralelismo e recursos dinâmicos

O runner usa `scripts/mutation-resource-profile.py` para detectar CPUs efetivamente disponíveis (afinidade e quota cgroup), memória disponível (incluindo limites cgroup) e espaço livre do filesystem escolhido para os scratch trees. O número de workers não é fixado em 8: é limitado pelo menor orçamento de CPU, RAM e disco. `MUTANTS_JOBS` pode pedir um número diferente, mas o valor aplicado nunca supera o teto seguro calculado. `CARGO_BUILD_JOBS` é calculado separadamente para evitar multiplicar workers do cargo-mutants por compiladores Cargo sem limite.

Os scratch trees ficam em `target/cargo-mutants-tmp` por padrão, no filesystem do checkout, em vez de depender de `/tmp`, que pode ser um `tmpfs` pequeno. O runner valida os filtros e executa o baseline puro uma vez antes de usar `--baseline=skip` no cargo-mutants, evitando repetir o build/teste limpo na cópia temporária. `CARGO_PROFILE_TEST_DEBUG` e `CARGO_PROFILE_DEV_DEBUG` são reduzidos para `0` apenas durante mutation testing para diminuir espaço e custo de link; use `MUTANTS_DEBUG_INFO=1` se precisar de símbolos básicos ao depurar falhas. Em falha de linker/SIGBUS, o log inclui `df -h`, `df -i` e `free -h`.
