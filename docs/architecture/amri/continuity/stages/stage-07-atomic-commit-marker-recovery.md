# Stage 7 — Atomic FACTSTR Commit, Marker e Recovery

**Estado:** Pendente; é o primeiro estágio que implementa persistência AMRI real.

## Objetivo

Confirmar todos os fatos do plano mais um marcador técnico de interação em um único append condicional, com garantia all-or-none, idempotência e resolução de resposta ambígua.

## Escopo

- Adapter de `AtomicInteractionCommitPort` para receber plano validado + mesmo token opaco original.
- Reconstruir exatamente a mesma `CompositeContextSpecification`/EventQuery que produziu o snapshot.
- Codificar cada fact tipado no schema/event type versionado correto.
- Adicionar marker técnico (`amri_interaction_committed.v1`, ainda proposto) na mesma batch do fato de negócio.
- Executar exatamente um `EventStore::append_if` para todos os fatos + marker.
- Projetar lookup/reconciliation do InteractionId/fingerprint e receipt.
- Implementar recuperação de resposta a partir do marker e dos fatos comprometidos.
- Mapear `ConditionalAppendConflict` e resultado ambíguo sem afirmar sucesso prematuramente.

## Invariantes

- Nenhum participant chama append.
- Nenhuma sequência de appends; não fazer um append por participant.
- A batch única inclui o marker e consequências de negócio cuja atomicidade foi exigida.
- Conflito/rejeição/falha definitiva anterior ao commit produz zero fatos da interaction.
- Marker não é uma capability de negócio e não contamina projeções comuns.
- Mesmo InteractionId + fingerprint diferente é rejeitado.
- Mesmo InteractionId + fingerprint idêntico retorna receipt já comprometido sem escrever de novo.
- Ledger HTTP existente não é transacionalmente atômico com FACTSTR; recovery authoritative é resolvido no marker/fatos AMRI.

## Testes obrigatórios com PostgreSQL/Testcontainers

- Um append call por commit attempt.
- Um batch persiste todos os fatos e marker, ou nenhum.
- Stale context token invalida batch.
- Duas conexões independentes preparadas com mesmo snapshot: no máximo uma confirma.
- RPU autônomo concorrente que muda fato relevante invalida AMRI.
- Same ID/same fingerprint vs same ID/different fingerprint.
- Concorrência de requests duplicadas.
- Falha de resposta após commit (fault injection); retry encontra marker e reconstrói receipt sem append duplicado.
- Token vinculado a StoreIdentity e spec; token de outro store/interaction/spec rejeita.
- Alteração fora do read set não gera conflito falso quando a semântica do adapter suporta essa distinção.

## Antes de implementar marker

Revalidar FACTSTR 0.5.2 append_if e query semantics na API instalada. Confirmar event type/version, payload canonical, índices/queries e efeito na versão do contexto. Não presumir semântica que não exista no código/contrato da versão usada.

## Gates de saída

Prova de all-or-none, CCC/race, idempotência e recovery com evidence logs Testcontainers. Sem essas provas, ADR-0034 não pode ser Accepted. Commit/push e atualização ADR/flow/evidence matrix.
