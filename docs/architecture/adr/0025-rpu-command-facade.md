# ADR-0025: Fachada pública de comandos e encapsulamento de Providers

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

A injeção de `ClockProvider` e `IdProvider` precisa continuar configurável no runtime e determinística nos testes. Contudo, expor `process_with_providers` de cada capability para os delivery mechanisms força o handler HTTP a conhecer detalhes de composição da Imperative Shell.

## Decisão

Introduzir `app::CommandRpus<S>` como fachada de comandos para a composição da aplicação. A fachada expõe operações por capability com request/response de domínio e retém internamente o EventStore e os Providers. Os handlers HTTP chamam a fachada; os módulos de capability mantêm `process_with_providers` como API `pub(crate)`. Providers continuam acessíveis na composição e nos testes, e o Functional Core recebe somente valores concretos.

Capturar os timestamps de ocorrência no início da execução lógica nos comandos que os consomem. Nos únicos comandos que têm retry CCC interno hoje (`CloseRfq` e `SubmitSupplierQuote`), os valores nondeterminísticos são capturados antes do laço e reutilizados nas tentativas; os comandos que não repetem internamente retornam conflito após a releitura/classificação, em vez de executar um retry cego.

## Consequências

- O HTTP adapter não precisa importar nem invocar Providers para comandar uma capability.
- O token de ownership do ledger HTTP usa um `UuidV4Provider` técnico separado da instância de `IdProvider` usada para IDs de domínio; retries e deduplicação da infraestrutura não consomem/alteram a sequência determinística dos IDs de domínio em testes.
- Testes de integração podem montar `CommandRpus` com relógio/IDs controlados e EventStore decorado.
- Uma futura capability que adote retry deve capturar valores lógicos uma única vez antes do laço, reler o contexto e refazer a decisão antes do append condicional.
- Chaves idempotentes HTTP controlam repetições entre requisições; não substituem CCC nem fazem parte da decisão de domínio.
