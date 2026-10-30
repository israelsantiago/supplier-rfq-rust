# ADR-0031: Finalização atômica do sourcing de RFQ via FACTSTR

- **Status:** Accepted; implementado na branch `main`
- **Data:** 2026-10-29
- **Escopo:** capability de comando aditiva; preserva os contratos e shells imperativos das RPUs legadas.
- **Implementação:** `src/capabilities/finalize_rfq_sourcing/` e adaptador `src/application_state/atomic_sourcing_factstr.rs`.

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

## Classificação arquitetural: capability/RPU, não Reactor

`FinalizeRfqSourcing` implementa uma única Domain Capability do tipo Command e seu próprio caminho RPU: carrega o contexto, aplica a decisão pura e confirma quatro consequências num único `append_if`. Quatro fatos produzidos por uma decisão não equivalem a quatro capabilities. Não há chamadas coordenadas a outras RPUs nem operações externas sequenciadas; por isso, esta implementação não é um Reactor.

A composição atual contém `AtomicSourcingFlow`, uma fachada pass-through que apenas delega para `FinalizeRfqSourcing::finalize`. Ela não coordena interações no sentido definido por Rico. O nome `Flow` é potencialmente ambíguo; a remoção dessa camada fina ou sua renomeação é uma limpeza futura de composição, não uma mudança de domínio e não faz parte desta ADR de implementação. Se surgir um processo externo que combine esta RPU com envio a ERP e outras capabilities, a coordenação desse processo poderá ser um Reactor separado.

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

## Evidência e gates

- `src/capabilities/finalize_rfq_sourcing/decision.rs` contém os testes puros da decisão: sucesso com quatro consequências e rejeições para RFQ/fornecedor/elegibilidade/seleção/cotação/itens/valores/validade.
- `tests/acceptance_atomic_sourcing.rs` exercita batch de quatro fatos, rejeição sem append, token de cotações obsoleto, conflito CCC após preparação por mudança de elegibilidade, nova cotação concorrente e segundo commit do mesmo snapshot preparado.
- `scripts/check-functional-core-branch-coverage.py` e `scripts/mutate-functional-core.sh` incluem a decisão pura da capability. Os runners puros excluem nominalmente os testes que usam PostgreSQL; a suíte integral continua executando-os.
- O workflow CI valida `verify-rpu-boundaries`, Testcontainers, branch coverage, OpenAPI, Mermaid e estado documental. Mutation testing é executado por `workflow_dispatch` ou agenda, não em todo push.

## Links

- [Fluxo de finalização atômica](../../flows/atomic-rfq-sourcing.md)
- [ADR-0006: CCC](./0006-command-context-consistency.md)
- [ADR-0016: estratégia de testes](./0016-test-strategy.md)
- [Rico Fritzsche — RPU](https://architecture.ricofritzsche.me/concepts/request-processing-units/)
- [Rico Fritzsche — CCC](https://architecture.ricofritzsche.me/concepts/command-context-consistency/)
- [FACTSTR](https://github.com/ricofritzsche/factstr)
