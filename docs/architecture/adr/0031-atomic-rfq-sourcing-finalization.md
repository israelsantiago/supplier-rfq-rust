# ADR-0031: Finalização atômica do sourcing de RFQ via FACTSTR

- **Status:** Accepted for implementation (additive flow; not yet merged into main)
- **Data:** 2026-10-29
- **Escopo:** novo fluxo de negócio; preserva os contratos e shells imperativos das RPUs existentes.

## Contexto

As RPUs de comando atuais (`OpenRfq`, `RegisterSupplier`, `SetSupplierEligibility`, `SubmitSupplierQuote`, `CloseRfq`) decidem e fazem commits CCC individuais. Encadeá-las por Reactor/orquestrador não garante que o commit seja indivisível; se uma etapa posterior falhar, as anteriores já podem ter persistido fatos.

O novo caso exige que a finalização de uma RFQ feche o sourcing, registre a adjudicação, crie o pedido no domínio e registre uma intenção durável de despacho externo — ou não persista nenhum desses efeitos.

## Decisão

1. Adicionar uma capability independente `FinalizeRfqSourcing`, não chamada pelas RPUs existentes nem exposta através de `CommandRpus`.
2. A nova shell lê uma única `EventQuery` CCC abrangendo estado de ciclo de vida da RFQ, todas as cotações dessa RFQ, registro/elegibilidade do fornecedor escolhido e fatos anteriores de adjudicação/pedido/despacho.
3. O Functional Core puro valida a seleção e calcula quatro fatos com timestamp/IDs explícitos.
4. A shell chama `append_if` uma única vez com os quatro `NewEvent`s e a versão observada da query completa. Não há append por etapa nem commit intermediário.
5. Não repetir automaticamente após conflito CCC: o comprador precisa rever o conjunto de cotações/estado antes de confirmar a escolha. O token de negócio `expected_quote_set_version` é derivado do maior `sequence_number` entre as cotações que foram efetivamente exibidas/revistas.
6. Os fatos são `rfq_closed.v1`, `rfq_awarded.v1`, `purchase_order_created.v1` e `purchase_order_dispatch_requested.v1`.
7. O último fato representa intenção durável, não prova de entrega ao ERP externo. Dispatcher/integração externa fica fora desta unidade transacional.

## Contexto CCC

O token CCC da leitura completa protege concorrências que ocorram depois da query. O token de cotação enviado pela interface protege contra uma seleção feita sobre uma lista já obsoleta antes de iniciar a operação. Os dois mecanismos são complementares; não substituir um por outro.

A query precisa corresponder à semântica das EventQueries do FACTSTR 0.5.2. Alterar tipos de fatos que afetam as decisões implica atualizar a query e os testes de contexto obsoleto.

## Alternativas consideradas

- **Reutilizar as shells legadas:** rejeitada; cada RPU confirma individualmente e a composição permite efeitos parciais.
- **Reactor chamando três RPUs:** rejeitada; ordem de chamadas não fornece atomicidade de persistência.
- **Três `append_if` consecutivos:** rejeitada; o primeiro append pode confirmar antes de um conflito posterior.
- **Uma batch `append_if` da capability composta:** aceita; alinha a fronteira de commit com o requisito “todas ou nenhuma”.
- **Transação relacional separada / dual-write FACTSTR + tabelas de domínio:** rejeitada para este caminho, pois introduz duas fontes autoritativas sem uma transação única.
- **Saga/compensação:** fora dos requisitos desta operação.

## Consequências

### Positivas

- Falha de regra de negócio: nenhum dos quatro fatos é gravado.
- Conflito CCC: nenhum dos quatro fatos é gravado.
- Fechamento, adjudicação, pedido e intenção de despacho compartilham o mesmo lote atômico.
- Os shells existentes permanecem intactos; a semântica antiga continua válida para os fluxos independentes.
- O evento de despacho permite reprocessamento posterior com idempotência downstream.

### Negativas / riscos

- A capacidade nova repete parte da interpretação de domínio e requer testes de contrato para não divergir das regras de `SubmitSupplierQuote`.
- O token de conjunto de cotações precisa corresponder exatamente ao conjunto que a UI permite selecionar; paginação/filtros parciais devem ser resolvidos antes de produção.
- A integração ao ERP não faz parte do commit FACTSTR; precisa de consumidor, retry e reconciliação próprios.
- A idempotência HTTP não compartilha atomicidade com o append FACTSTR; resposta ambígua após timeout exige política de reconciliação.
- A query abrangente pode gerar conflitos para fatos que concorrem com o sourcing; deve ser medida e mantida tão estreita quanto as invariantes permitem.

## Testes e gates

Manter todos os testes existentes. Adicionar testes unitários para cada ramo do Functional Core e aceitação com Testcontainers/PostgreSQL para batch bem-sucedido, zero efeitos em rejeição, conflito de elegibilidade após leitura, quote set obsoleto, dupla preparação/commit e concorrência quote-vs-finalização. Adicionar o novo arquivo de decisão ao gate de 100% de branches. Executar fmt/check/Clippy/testes/OpenAPI/Mermaid/guards já definidos em ADR-0016 e no workflow de CI.

## Links

- [Fluxo de finalização atômica](../../flows/atomic-rfq-sourcing.md)
- [ADR-0006: CCC](./0006-command-context-consistency.md)
- [ADR-0016: estratégia de testes](./0016-test-strategy.md)
- [Rico Fritzsche — RPU](https://architecture.ricofritzsche.me/concepts/request-processing-units/)
- [Rico Fritzsche — CCC](https://architecture.ricofritzsche.me/concepts/command-context-consistency/)
- [FACTSTR](https://github.com/ricofritzsche/factstr)
