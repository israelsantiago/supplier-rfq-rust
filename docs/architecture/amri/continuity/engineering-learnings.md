# Engineering learnings — AMRI

## 2026-11-20 — Stage 5B.2 final re-verification

- Recibo final: 17/17 gates PASS, zero FAIL.
- Onze gates foram executados novamente; seis históricos reutilizados
  após validação de integridade dos logs.
- Mutation foi SKIPPED pela política da subetapa, não PASS.
- Evidência-only publicada não equivale ao encerramento automático da fase.
- A ausência de status CI no recibo não permite declarar CI remoto aprovado.

## Publishers e automação

- Estado persistente deve ser criado antes dos gates e operações de publicação.
- `status` deve continuar funcional depois de uma falha parcial.
- `git diff --cached` isolado não é prova suficiente do índice; cruzar
  porcelain v2, `git ls-files --stage`, blobs e allowlist.
- O patch completo precisa passar `git apply --stat` e `git apply --check`
  no baseline exato. Hash não prova aplicabilidade.
- Pacotes são imutáveis e versionados; validar checksums internos e externos,
  inclusive após extração. Não incluir `__pycache__`.
- Logs originais não devem ser alterados; publicar cópias sanitizadas.
- Scripts são executados com `bash`, registram branch/HEAD e códigos de saída,
  preservam falhas e não modificam opções persistentes do shell.

## Arquitetura e sequência AMRI

- Stage 5B.3-R é documental; não criar abstrações sem consumidor.
- `FinalizeRfqSourcing` / `AtomicSourcingFlow` do ADR-0031 não é o Reactor AMRI.
- Close e Award observam o mesmo snapshot e são independentes.
- Create Purchase Order depende de `AwardedSupplierQuote`.
- Identity, fingerprint e timestamps permanecem estáveis em retries CCC;
  snapshot/token são renovados e resultados anteriores descartados.
- Providers são read-only; ausência de evidência e falha operacional são
  estados distintos. Não adicionar retry automático ou failover.
- Stage 5 exclui escrita, `append_if`, marker e recovery.
- ADR-0034 permanece Proposed até os gates posteriores.
