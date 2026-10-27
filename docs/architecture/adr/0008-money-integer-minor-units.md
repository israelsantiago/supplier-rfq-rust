# ADR-0008: Valores monetários em unidades menores inteiras

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

Cálculos de cotação não podem depender de arredondamento binário de ponto flutuante.

## Decisão

Representar preço e total em centavos (`i64`) e usar operações aritméticas verificadas, como `checked_mul` e `checked_add`.

## Consequências

As regras de moeda, escala e limites devem ser validadas no domínio. Não misturar moedas numa comparação de valores sem regra explícita.
