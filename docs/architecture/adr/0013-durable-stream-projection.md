# ADR-0013: Projeção via stream durável e consistência eventual

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

Consultas de resumo não devem reconstruir todo o histórico de fatos em cada leitura.

## Decisão

Manter a projeção `rfq_summary_projection` por meio do stream durável `rfq-summary-projection-v1`; o log de fatos permanece autoritativo.

## Consequências

A projeção pode estar temporariamente atrasada. Handlers devem confirmar a transação antes de o cursor avançar. A estratégia de múltiplas instâncias/consumidores deve ser definida antes de escalar horizontalmente.
