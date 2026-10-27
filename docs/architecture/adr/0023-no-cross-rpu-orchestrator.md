# ADR-0023: Sem orquestrador transversal de RPUs no fluxo atual

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

As operações implementadas correspondem a capabilities isoladas e não demonstram uma interação que exija coordenar múltiplas RPUs.

## Decisão

Não introduzir um Reactor/orquestrador global até haver uma interação de usuário ou workflow real que demande essa coordenação.

## Consequências

Evita complexidade prematura; se surgirem workflows multi-capability, rever esta decisão e explicitar propriedade, durabilidade e falhas parciais.
