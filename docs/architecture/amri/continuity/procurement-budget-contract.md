# Procurement Budget — contrato proposto v1

**Status:** Proposed; não aprovado para escrita/produção.
**Escopo:** contrato de entrada de alocação e reserva usado pelo piloto Stage 5C `supply-chain.procurement-commit`.

## 1. Ownership e produtor

O baseline do repositório não contém uma capability de alocação orçamentária nem identifica um owner de negócio ou produtor de `procurement_budget_allocated.v1`. O código não deve inventar uma pessoa, equipe, ERP ou sistema de origem. O owner accountable deve ser formalmente nomeado pelo domínio Financeiro/Procurement; esse owner decide o sistema produtor e a autoridade de emissão.

O consumidor técnico proposto é `ReserveProcurementBudgetParticipant`, por meio da projeção do `CompositeContextReader`. A reserva proposta é `procurement_budget_reserved.v1`; não há adapter de persistência nem produtor de alocação nesta etapa.

## 2. Contratos de evento

| Evento | Papel | Campos do contrato v1 |
|---|---|---|
| `procurement_budget_allocated.v1` | Pré-condição imutável de alocação | `budget_id`, `currency`, `allocated_amount_cents`, `allocated_at` |
| `procurement_budget_reserved.v1` | Consequência proposta de reserva | `reservation_id`, `budget_id`, `rfq_id`, `supplier_id`, `currency`, `reserved_amount_cents`, `reserved_at` |

Valores monetários usam minor units inteiras (`i64`) e devem ser positivos. O código valida moeda como três letras ASCII maiúsculas; isso não equivale a uma validação completa de catálogo ISO 4217. Payloads rejeitam campos desconhecidos.

## 3. Política funcional v1 implementada

1. Um único fato de alocação por `budget_id`; duplicidade é erro, não uma revisão/ajuste implícito.
2. Uma reserva histórica por par `(budget_id, rfq_id)`, independentemente do supplier. Um `reservation_id` não pode ser reutilizado no histórico do mesmo budget.
3. Reservas existentes são somadas com aritmética checked; moeda deve corresponder à alocação; saldo nunca pode ficar negativo.
4. A reserva pedida deve ser positiva e não pode exceder o saldo calculado.
5. A decisão é pura e produz apenas um fato proposto. Não aloca, não ajusta e não persiste.

## 4. Cancelamento, release e ajustes — não suportados em v1

Não existe `procurement_budget_reservation_released.v1`, evento de ajuste de alocação, release parcial ou operação de cancelamento com restituição. Uma reserva já registrada permanece ocupando o orçamento; o mesmo par `(budget_id, rfq_id)` não pode ser reservado novamente. Nenhum saldo deve ser considerado liberado por cancelamento inferido, status externo ou desaparecimento de snapshot.

Essa é uma restrição fail-closed do protótipo, não uma afirmação de que seja a política comercial desejada. Antes de persistir `procurement_budget_reserved.v1`, o owner deve decidir release total/parcial, motivo/evidência de cancelamento, re-reserva, concorrência, correção de alocação e fechamento/expiração do budget. Uma versão de contrato posterior deverá modelar as decisões aprovadas sem alterar retroativamente os fatos v1.

## 5. Gates de governança

| Gate | Situação |
|---|---|
| Owner accountable nomeado | Pendente — não consta da fonte |
| Produtor autorizado de `procurement_budget_allocated.v1` | Pendente |
| Granularidade e unicidade de `budget_id` | Proposta: uma alocação imutável por ID; requer ratificação |
| Política de alteração/redução de alocação | Não suportada em v1; decisão de negócio pendente |
| Política de cancelamento/release/re-reserva | Não suportada em v1; decisão de negócio pendente |
| Aprovação para Stage 7/produção | Bloqueada até gates acima |

**Stage 6 pode avançar** com validação estrutural, composição e commit plan imutável em memória, mantendo o contrato marcado como Proposed. **Stage 7 não pode persistir** esse contrato nem ativar o fluxo produtivo até aprovação formal. ADR-0034 permanece `Proposed`.

## 6. Evidência e testes

Os testes do Functional Core verificam allocation ausente/duplicada, moeda/valor inválidos, identidade repetida, reserva duplicada por `(budget_id, rfq_id)` mesmo quando o supplier difere, soma overflow, orçamento já sobrealocado e saldo insuficiente. Os testes de projeção verificam payload inválido, evento de budget fora do escopo e PO v2 malformado/fora do RFQ.
