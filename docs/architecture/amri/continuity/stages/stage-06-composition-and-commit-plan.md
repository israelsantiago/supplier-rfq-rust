# Stage 6 — Composition Policies e Validated Commit Plan

**Estado:** Pendente; depende do Reactor/participant flow da Stage 5.

## Objetivo

Impedir que outcomes localmente válidos produzam um conjunto globalmente inválido e selar uma representação imutável/determinística para a futura confirmação. O Core continua genérico; as regras cruzadas são de Supply Chain.

## Estrutura

- Structural Validator genérico AMRI: participantes esperados, identidade/owner de outcome, fact type allowlist, IDs únicos, dependências satisfeitas, complete set e ordem determinística.
- Composition Policies puras de domínio: validações entre `RfqClosedFact`, `RfqAwardedFact`, `PurchaseOrderCreatedFact`, eventual intenção de despacho e request/context.
- Plan builder: construir/sealar batch tipado e seu fingerprint usando canonical encoding versionado da aplicação.
- Separar `plan fingerprint` de `request fingerprint`; definir qual material lógico cada um protege.

## Regras cross-capability mínimas

- Todos os fatos pertencem ao mesmo RFQ/request Interaction.
- Award referencia a cotação realmente selecionada e o fornecedor escolhido.
- A quote pertence ao RFQ/fornecedor e a revisão/versão considerada.
- PO deriva do award e preserva currency, totals, delivery terms e item lines.
- Item IDs/quantidades/line totals e total consolidado são consistentes.
- Qualquer dispatch intent referencia exatamente o PO criado.
- Não produzir duplicidade/resultado conflitante para uma RFQ já finalizada.
- Nenhum fact type fora da allowlist ou com identidade duplicada.

## Testes

- Matriz positiva de composição compatível.
- Matrix negativa por cada vínculo (RFQ, supplier, quote, award, PO, currency, item, amount, dispatch).
- IDs duplicados, fact type não autorizado, outcomes ausentes/sobrando, dependency missing, plan tampered.
- Property tests para combinações de itens/quantidades/valores.
- Fatos e outcomes mantêm ordem determinística sob input/permutação de coleções.

## Não fazer

Sem persistence, port commit, `append_if`, interaction marker, recovery, retries externos ou mutação de FACTSTR. A plan ainda é apenas um objeto validado.

## Gates de saída

Nenhuma composição inválida produz plan; tests puros/property completos; `ValidatedCommitPlan` genérico não depende de RFQ. Commit/push e atualização de matriz de evidências.
