# Mutation testing do Functional Core

A cobertura de ramos comprova que as alternativas foram exercitadas; ela não prova, sozinha, que as assertions detectam uma decisão errada. `cargo-mutants` complementa os testes alterando temporariamente o comportamento de funções e verificando se a suíte detecta a mudança.

## Executar

```bash
bash scripts/mutate-functional-core.sh
```

Se necessário, o runner instala `cargo-mutants` automaticamente na primeira execução; é necessário acesso à rede para baixar a ferramenta. O runner foca em `generate_consequences.rs`, `build_context.rs` e `submit_supplier_quote/decision.rs`, executando os testes unitários da library. O Docker precisa estar ativo porque a suíte unitária da projeção também usa Testcontainers. Para ajustar duração e paralelismo, use `MUTANTS_TIMEOUT=120 MUTANTS_JOBS=4`.

Os artefatos de `mutants.out/` são locais e ficam ignorados pelo Git. Priorize mutantes `MISSED`/`NOT CAUGHT`: leia a alteração gerada, determine se ela é semanticamente relevante e acrescente uma assertion objetiva. Não trate mutantes não compiláveis como cobertura obtida, nem exija que mutações semanticamente equivalentes sejam mortas. Mutation testing pode levar vários minutos; não é executado no CI rápido padrão.

Referência oficial: [cargo-mutants](https://mutants.rs/), [filtragem por arquivo](https://mutants.rs/skip_files.html), [interpretação de resultados](https://mutants.rs/using-results.html).
