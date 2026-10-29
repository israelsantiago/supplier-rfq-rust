# ADR-0027: Testes de falha, concorrência e estados da idempotência HTTP

- **Status:** Accepted
- **Data:** 2026-10-28
- **Relacionados:** ADR-0016, ADR-0022, ADR-0025, ADR-0026

## Contexto

O middleware já possui fingerprint, claim de chave, replay de resposta e estados `in_progress`, `completed` e `unknown`. Entretanto, os testes anteriores cobriam sobretudo replay sequencial de sucesso e reutilização da chave com payload divergente. A garantia é crítica na janela em que o fato pode ter sido gravado em FACTSTR e a resposta ainda não foi persistida no ledger SQL.

FACTSTR e o ledger HTTP usam recursos e transações independentes. Não há uma transação distribuída que inclua append do fato e gravação da resposta; portanto não se pode declarar exactly-once distribuído. A estratégia segura é impedir reexecução automática enquanto a saída é incerta.

## Decisão

Complementar os testes com PostgreSQL/Testcontainers e testes HTTP de aceitação para:

1. duas claims simultâneas da mesma chave: um único owner;
2. requisição concorrente enquanto a claim está `in_progress`;
3. transição para `unknown` após falha ao persistir a resposta;
4. `unknown` continua bloqueando mesmo com `expires_at` no passado;
5. somente `completed` expirado pode ser reclaimado;
6. falha do ledger antes de executar o comando retorna indisponibilidade e não produz fato;
7. falha ao marcar `completed` depois do append FACTSTR mantém uma única ocorrência do fato e a tentativa repetida retorna `idempotency_outcome_unknown`;
8. concorrência HTTP com a mesma chave produz no máximo um fato de domínio;
9. replay sequencial de sucesso e resposta de negócio 4xx, além da incompatibilidade de fingerprint, continuam cobertos;
10. JSON semântico reordenado faz replay; chave inválida/duplicada e corpo acima de 2 MiB são rejeitados antes do comando;
11. path, query string e Content-Type fazem parte do fingerprint;
12. resposta acima de 4 MiB leva a `unknown` e impede replay seguro.

O ledger recebe pool limitado independente do pool da projeção. A migration original em `migrations/` permanece byte a byte para preservar o histórico SQLx existente. Uma cópia idempotente em `migrations_idempotency/` é usada pelo inicializador do ledger, que guarda versão e checksum em `http_idempotency_migrations` sob advisory lock. O código falha em caso de divergência de checksum. Ambos os pools podem conectar ao mesmo banco PostgreSQL sem compartilhar conexões.

Um guardrail estático compara os parâmetros `Idempotency-Key` em `docs/openapi.yaml` com os seis endpoints de comando, incluindo `POST /api/rfqs/{rfq_id}/finalize-sourcing`; o header não deve aparecer em `GET /api/rfqs/{rfq_id}/quotes`, pois o middleware não aplica idempotência a consultas.

## Consequências

**Positivas:** estados e transições centrais do ledger ganham evidência reproduzível; evita-se regressão que reexecute comandos sob resultado incerto; o OpenAPI passa a corresponder ao middleware; a projeção deixa de emprestar seu pool para a infraestrutura de idempotência.

**Limites restantes:** trigger SQL reproduz falha de persistência, mas não equivale a matar o processo em qualquer instrução, failover, partição de rede ou restore de backup. Operação requer monitorar e reconciliar chaves `in_progress`/`unknown` contra fatos autoritativos. A chave é opcional; sem ela o protocolo de deduplicação HTTP não se aplica. Em futura autenticação/tenancy, o principal/tenant deve compor o escopo da chave.

## Critérios de validação

- `cargo test --all-targets --all-features --locked`;
- teste de máquina de estados `ledger_state_machine_fails_closed_and_expires_only_completed_records`;
- testes de aceitação `http_idempotency_*`;
- `oversized_response_marks_key_unknown_and_blocks_retry`;
- `python3 scripts/verify-idempotency-openapi.py` e lint OpenAPI;
- confirmação de que a migration histórica foi mantida e a migração dedicada usa checksum imutável.

## Referências

- [SQLx 0.8 migrations](https://docs.rs/sqlx/0.8.6/sqlx/migrate/struct.Migrator.html)
- [Rico Fritzsche — Command Context Consistency](https://architecture.ricofritzsche.me/concepts/command-context-consistency/)
