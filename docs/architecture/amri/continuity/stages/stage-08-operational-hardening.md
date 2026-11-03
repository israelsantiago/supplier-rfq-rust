# Stage 8 — Hardening, Operabilidade e Performance

**Estado:** Pendente; depende do Stage 7 funcional e de evidências de atomicidade/recovery.

## Objetivo

Tornar a interação observável, limitada em recursos, recuperável e mensurável. Não usar benchmark para justificar enfraquecimento de invariantes.

## Escopo

- Tracing estruturado por InteractionId e participant id.
- Medir duração por fase, contexto (nº de fact types/records), número de facts, batch size e resultado de commit.
- Telemetria de conflitos, retries, external evidence age/expiry e recovery path.
- Limites explícitos para participants, read set, batch e payload.
- Timeouts/cancellation e circuit/failure policy de Providers.
- Retry bounded apenas para caminhos declaradamente replay-safe; nunca repetir decisão humana sem nova revisão.
- Fault injection para timeout, conexão perdida, panic controlado, crash depois do commit e erro de Provider.
- Runbook para commit ambíguo, marker lookup, reservation release, reconciliação e DLQ/outbox downstream.
- Benchmark com carga e contenção realistas; medir p50/p95/p99, throughput, conflitos e lock waits.

## Restrições

- Não manter transação/lock aberta durante Provider externo ou interação humana.
- Não tratar log/metric como autoridade de persistência.
- Não fazer retry infinito nem replay cego de ação externa.
- Não inserir dispatch real antes do commit authoritative.

## Gates de saída

Runbooks exercitados, falhas reconciliadas, limites de performance publicados, resultados reproduzíveis e tradeoffs de contention documentados. Atualizar status de risco e ADR, sem claim não comprovado de produção readiness.
