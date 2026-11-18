# ADR-0024: Seam de teste protegido por feature

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

Testes CCC precisam preparar decisões e separar preparação de commit sem ampliar desnecessariamente a API pública de produção.

## Decisão

Expor `prepare`/`commit_prepared` pela feature opt-in `test-support` e usar `cargo test --all-features` na validação completa.

## Consequências

O build de produção padrão não deve depender desse seam. Toda mudança deve preservar a compilação com e sem features.

<!-- ADR-0024-PROVIDER-TEST-SUPPORT:START -->

## Atualização — Fakes de Providers AMRI

Os Fakes dos ports ERP Reference, Compliance e Tax ficam disponíveis somente sob `cfg(test)` ou pela feature opt-in `test-support`. Reexports usados por testes integrados permanecem explicitamente feature-gated. Build de produção padrão não amplia a API pública e não liga WireMock/Testcontainers ao Functional Core.
<!-- ADR-0024-PROVIDER-TEST-SUPPORT:END -->
