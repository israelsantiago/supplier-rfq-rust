# ADR-0032: Semântica inclusiva da validade de cotações

- **Status:** Accepted para a implementação de referência
- **Escopo:** decisões puras `SubmitSupplierQuote` e `FinalizeRfqSourcing`

## Contexto

As duas capabilities têm `valid_until` como uma data de calendário (`NaiveDate`) e recebem um instante explícito `DateTime<Utc>` como entrada da decisão pura. Ambas rejeitam somente quando `valid_until < now.date_naive()`. O mutation testing substituiu `<` por `<=` nos dois pontos e os mutantes passaram: faltava uma assertion de fronteira que comprovasse a aceitação no próprio dia de validade.

## Decisão

1. `valid_until` é **inclusivo**: a cotação é válida durante a data especificada; só está expirada quando `valid_until` é anterior à data da decisão.
2. Exemplos de comportamento: ontem → rejeitar; hoje → aceitar; amanhã → aceitar.
3. Para esta implementação de referência, a data da decisão é explicitamente a data UTC extraída do instante recebido (`now.date_naive()`). Não se consulta relógio nem timezone do sistema dentro do Functional Core.
4. O instante usado na decisão continua sendo recebido como entrada determinística. O Core calcula e retorna fatos/consequências; não executa efeitos de I/O.
5. Se uma implantação contratual exigir calendário comercial em fuso diferente de UTC, isso precisa ser decidido antes de produção. A shell deverá converter o instante capturado para uma data comercial explicitamente definida (ou passar essa data como entrada adicional do Core) e os testes de fronteira deverão seguir o contrato aprovado. Não usar o timezone local implicitamente.

## Testes e evidência

- `SubmitSupplierQuote`: teste de aceite com validade hoje e amanhã; teste existente de rejeição para ontem.
- `FinalizeRfqSourcing`: teste de validade hoje que verifica a decisão positiva e os quatro fatos: fechamento da RFQ, adjudicação, criação do pedido e intenção de despacho. O teste existente para cotação expirada verifica ontem; o teste de decisão válida cobre data futura.
- Estes são testes puros do Functional Core: sem Docker, Postgres, EventStore, adapters, HTTP ou Providers.
- Mutation testing mira as funções puras/de decisão. Testes de shell, persistência e CCC permanecem em seus próprios gates e não fazem parte do escopo de mutações do Core.

## Consequências

A semântica de igualdade fica documentada e verificável. A alteração não muda lógica de produção; ela fortalece a evidência de teste para a regra atual. A decisão sobre timezone comercial continua sendo um requisito explícito antes de uma eventual implantação real.

## Referências arquiteturais

- [Functional Core / Imperative Shell](https://architecture.ricofritzsche.me/concepts/functional-core-imperative-shell/)
- [Request Processing Units](https://architecture.ricofritzsche.me/concepts/request-processing-units/)
- [Providers](https://architecture.ricofritzsche.me/concepts/providers/)
