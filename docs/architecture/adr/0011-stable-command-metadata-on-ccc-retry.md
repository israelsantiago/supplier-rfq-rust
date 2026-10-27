# ADR-0011: Metadados estáveis durante retries internos de CCC

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

Refazer a leitura do contexto não deve criar uma nova identidade nem alterar incidentalmente o horário da mesma execução lógica.

## Decisão

Capturar `quote_id` e `submitted_at` uma única vez antes do laço de retries de `SubmitSupplierQuote`. Reutilizar os mesmos valores em todas as preparações internas. Aplicar a mesma política de timestamp às capabilities com retries.

## Consequências

O timestamp padrão representa o momento lógico de submissão/início definido pelo comando, não necessariamente o instante do commit. Se o requisito fiscal/comercial exigir validade no instante da aceitação, separar `submitted_at` de `decision_at`/`accepted_at`, validar `valid_until` contra o último e manter `submitted_at` estável. Essa alternativa exige alteração de regra e testes; não é adotada implicitamente.
