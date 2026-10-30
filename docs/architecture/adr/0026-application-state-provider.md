# ADR-0026: Application State Provider como fronteira do estado autoritativo

- **Status:** Accepted
- **Data:** 2026-10-28
- **Substitui:** nenhuma
- **Relacionados:** ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0016, ADR-0025

## Contexto

As RPUs já separavam leitura, Functional Core e append condicional, porém os shells de várias capabilities dependiam diretamente de `factstr::{EventStore, EventQuery, EventFilter, EventRecord, NewEvent}`. Isso propagava a tecnologia de armazenamento pelo código de aplicação e fazia os módulos de capability conhecerem o contrato operacional do adaptador FACTSTR.

A semântica de contexto não pode ser alterada por uma abstração cosmética: as consultas de identidade usam alternativas (`ID OR referência`); `SubmitSupplierQuote` combina os contextos de RFQ, fornecedor, elegibilidade e cotações; e o append precisa permanecer atomicamente condicional à versão que fundamentou a decisão (CCC). A atribuição de elegibilidade continua dependendo apenas do fato de cadastro, não do histórico de elegibilidade, conforme ADR-0019.

## Decisão

Introduzir `ApplicationStateProvider`, uma porta definida pela aplicação com:

- consultas tipadas por contexto (`StateQuery`), nomeadas segundo a capability/decisão;
- fatos de domínio tipados (`ApplicationFact`/`StoredFact`) e snapshots de leitura;
- um `ContextVersion` opaco para o consumidor, sem expor a representação numérica do FACTSTR;
- `append_if` que recebe o contexto, a versão esperada e as consequências tipadas.

`FactstrApplicationStateProvider` implementa o contrato geral `ApplicationStateProvider` e traduz `StateQuery`/`ApplicationFact` para `EventQuery`, `EventFilter`, `EventRecord` e `NewEvent`, além de decodificar payloads e converter conflitos de append para `StateWriteError::ContextChanged`. Payloads inválidos ou tipos de fato desconhecidos falham de forma explícita; não viram contexto parcial silencioso.

`FinalizeRfqSourcing` usa adicionalmente a porta especializada `AtomicSourcingStateProvider`, implementada por `FactstrAtomicSourcingStateProvider` em `src/application_state/atomic_sourcing_factstr.rs`. Essa porta expressa o contexto composto da seleção de sourcing e grava as quatro consequências numa única batch. Na composição de produção, os dois adaptadores recebem a mesma instância compartilhada de `PostgresStore`: são contratos especializados para necessidades diferentes, não fontes autoritativas diferentes. A decisão de mantê-los separados e evitar uma abstração genérica prematura está registrada na [ADR-0033](./0033-application-state-provider-specialized-ports.md).

As fachadas `CommandRpus` e `QueryRpus` recebem `Arc<dyn ApplicationStateProvider>`. Handlers HTTP não recebem o `PostgresStore` autoritativo diretamente. A projeção de resumos continua sendo um read model SQLx separado: esta decisão não a transforma em fonte autoritativa nem obriga a encaixá-la na mesma porta.

## Garantias de compatibilidade

- Preservar as queries existentes e sua semântica OR/AND exatamente, incluindo o escopo CCC específico de cada capability.
- Preservar tipos/versionamento de fatos, payloads, regras de domínio, respostas e códigos HTTP.
- Preservar captura única de ID/timestamp por execução lógica e o número de tentativas CCC já definido.
- Preservar sequência de eventos retornada em `ListSupplierQuotes` para o campo `sequence_number`.
- Testar o adaptador com PostgreSQL real, incluindo leitura tipada e recusa de append com versão obsoleta.

## Consequências

**Positivas:** as capabilities deixam de importar tipos FACTSTR em código de produção; as queries nomeadas são revisáveis como contrato da aplicação; outra implementação de estado pode ser introduzida sem reescrever decisões e shells.

**Custos/limites:** o adaptador contém mapeamento explícito entre fatos e tipos serializados, que deve evoluir junto com os fatos versionados. A porta não reduz o custo de leitura nem define uma tecnologia de armazenamento substituta. O token de versão é opaco para as RPUs, mas o adaptador FACTSTR o valida para evitar usar um token de outro provider.

## Evidência e validação

- `scripts/verify-application-state-boundary.sh` verifica estaticamente que capabilities de produção não importam tipos/tipos de operação FACTSTR.
- `invalid_known_fact_fails_closed` e `unknown_fact_type_fails_closed` verificam decodificação no adaptador.
- `provider_reads_typed_facts_and_rejects_stale_conditional_append` usa Testcontainers/PostgreSQL para provar leitura tipada, gravação e conflito CCC.
- Os testes de Functional Core dos builders cobrem fatos tipados e preservam `sequence_number` em `ListSupplierQuotes`.
- `cargo test --all-targets --all-features --locked`, Clippy e os testes de CCC existentes devem passar antes do merge.

## Referências

- [Rico Fritzsche — Application State Provider](https://architecture.ricofritzsche.me/concepts/application-state-provider/)
- [ADR-0033 — portas especializadas para capabilities compostas](./0033-application-state-provider-specialized-ports.md)
- [Rico Fritzsche — Request Processing Units](https://architecture.ricofritzsche.me/concepts/request-processing-units/)
- [Rico Fritzsche — Functional Core / Imperative Shell](https://architecture.ricofritzsche.me/concepts/functional-core-imperative-shell/)
- [Rico Fritzsche — Command Context Consistency](https://architecture.ricofritzsche.me/concepts/command-context-consistency/)
- [FACTSTR](https://github.com/ricofritzsche/factstr)
