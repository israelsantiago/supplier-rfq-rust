# ADR-0019: Elegibilidade como atribuição de estado

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

A capability atual atribui o estado de elegibilidade e não modela uma transição condicionada ao status anterior.

## Decisão

`SetSupplierEligibility` exige cadastro do fornecedor, mas não inclui o histórico de elegibilidade na query CCC. O último fato aceito define o status atual.

## Consequências

Duas atribuições concorrentes podem ser aceitas em ordem de commit. Se o negócio exigir transições condicionais, mudar a regra, o CCC e os testes juntos.
