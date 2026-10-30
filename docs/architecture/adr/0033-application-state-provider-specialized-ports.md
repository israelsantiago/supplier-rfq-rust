# ADR-0033: Portas especializadas de Application State para capabilities com contratos distintos

- **Status:** Accepted
- **Data:** 2026-10-30
- **Relacionados:** ADR-0003, ADR-0004, ADR-0006, ADR-0026, ADR-0031

## Contexto

A implementação mantém duas portas de acesso ao mesmo Application State autoritativo:

- `ApplicationStateProvider`, usada pelas RPUs legadas para consultas nomeadas e append condicional de consequências tipadas;
- `AtomicSourcingStateProvider`, usada exclusivamente por `FinalizeRfqSourcing` para carregar um contexto composto de sourcing e confirmar quatro consequências na mesma batch.

Os dois adaptadores FACTSTR (`FactstrApplicationStateProvider` e `FactstrAtomicSourcingStateProvider`) recebem a mesma instância compartilhada de `PostgresStore` na composição de produção. A existência de duas interfaces não representa dois stores, duas fontes de verdade ou uma projeção adicional.

## Decisão

1. Manter `ApplicationStateProvider` como contrato geral das capabilities que já o utilizam.
2. Manter `AtomicSourcingStateProvider` como contrato especializado, pertencente à capability `FinalizeRfqSourcing`, enquanto seu modelo de leitura e sua operação de append em lote forem distintos.
3. Manter os adaptadores FACTSTR no limite de infraestrutura; nenhuma decisão de domínio é movida para os adaptadores.
4. Exigir que leitura e `append_if` reconstruam exatamente a mesma query CCC e usem a versão de contexto observada.
5. Não introduzir uma interface genérica ou hierarquia de Providers apenas para eliminar duplicação superficial. Reavaliar a unificação somente se houver semântica compartilhada real e testes que provem a preservação do contrato CCC.

## Consequências

**Positivas:** a capability composta possui um contrato explícito para seu contexto e seu lote atômico; as RPUs legadas preservam seus contratos; ambos os caminhos compartilham o mesmo registro autoritativo FACTSTR; o adaptador continua responsável pela tradução de storage, não pela decisão.

**Custos/limites:** existe alguma duplicação de tradução/desserialização entre os adaptadores. Ela deve ser mantida explícita e coberta por testes, em vez de ser escondida numa abstração genérica que misture contratos diferentes. A duplicação deve ser reavaliada quando novos fatos/capabilities exigirem manutenção comum substancial.

## Evidência e validação

- `scripts/verify-application-state-boundary.sh` verifica que os módulos de capability não dependem diretamente dos tipos/implementação FACTSTR.
- `tests/acceptance_atomic_sourcing.rs` exercita leitura do contexto composto e append condicional do lote de quatro fatos contra PostgreSQL/FACTSTR.
- Os testes de CCC das RPUs legadas continuam cobrindo os contratos de `ApplicationStateProvider`.
- `scripts/verify-rpu-boundaries.sh` e a suíte completa são gates de regressão; não se deve incluir Testcontainers no Functional Core ou no mutation testing puro.

## Referências

- [Rico Fritzsche — Application State](https://architecture.ricofritzsche.me/concepts/application-state/)
- [Rico Fritzsche — Application State Provider](https://architecture.ricofritzsche.me/concepts/application-state-provider/)
- [Rico Fritzsche — Request Processing Units](https://architecture.ricofritzsche.me/concepts/request-processing-units/)
- [ADR-0026 — Application State Provider](./0026-application-state-provider.md)
- [ADR-0031 — Finalização atômica de sourcing](./0031-atomic-rfq-sourcing-finalization.md)
