# Stage 0 — Baseline e proteção

**Estado:** Concluída anteriormente.\
**Natureza:** inventário, proteção e evidência; sem alteração funcional da aplicação.

## Objetivo

Definir um ponto de partida verificável, inventariar RPUs, ports, `FinalizeRfqSourcing`, scripts e testes, documentar o que não pode mudar e separar evidência local de CI.

## Entregáveis

- SHA/ref de base e branch de trabalho.
- Inventário de capacidades e suas shells/Functional Cores.
- Inventário dos providers/state ports e adapters FACTSTR.
- Baseline de `FinalizeRfqSourcing` como capability única com quatro fatos e um append condicional.
- Inventário de testes puros, acceptance/CCC/Testcontainers e mutation runners.
- Lista de arquivos/scripts que protegem fronteiras.
- Backup/export para recuperação e logs reproduzíveis.

## Regras de proteção

- Não modificar RPU autônomo, schema/event contracts, migration nem API pública nesta etapa.
- Não renomear branch ou mover commits como conveniência.
- Registrar limitações e divergências do ambiente local.

## Gates e saída

Baseline reproduzível; working tree conhecido; CI/checks identificados; zero alteração funcional. Reconsultar o estado remoto antes de qualquer fase seguinte.

## Evidência histórica

O baseline arquitetural e o inventário permanecem no `docs/architecture/amri/baseline.md`, nos ADRs e no `implementation-plan.md`. Um novo chat deve validar a situação atual, não assumir que hashes antigos continuam HEAD.
