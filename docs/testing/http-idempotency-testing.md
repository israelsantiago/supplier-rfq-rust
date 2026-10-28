# Testes de confiabilidade da idempotência HTTP

## Máquina de estados

- `in_progress`: uma única requisição detém o token owner; repetição concorrente não executa o comando.
- `completed`: status, headers de representação permitidos e corpo são reproduzidos para fingerprint compatível dentro do prazo de retenção.
- `unknown`: o comando pode ter persistido no FACTSTR, mas a resposta não foi guardada com segurança; replay automático fica bloqueado.
- Só entradas `completed` expiradas são elegíveis para remoção/reuso. `in_progress` e `unknown` exigem reconciliação.

## Matriz de regressão

| Cenário | Teste | Invariante observada |
|---|---|---|
| Claims simultâneas | `ledger_state_machine_fails_closed_and_expires_only_completed_records` | Exatamente um owner ganha a chave. |
| Repetição durante execução | Mesmo teste | Retorna `409 idempotency_in_progress`, sem segunda execução. |
| Falha de persistência da resposta | Mesmo teste (trigger SQL) | Falha ao gravar `completed` e o `unknown` permanece bloqueado. |
| `unknown` expirado | Mesmo teste | Não é automaticamente removido. |
| `completed` expirado | Mesmo teste | Pode ser reclaimado. |
| Ledger indisponível antes do comando | `http_idempotency_store_unavailable_prevents_command_execution` | Retorna 503 e não grava fato de domínio. |
| Replay sequencial | `http_idempotency_replays_the_original_success_response` | Reproduz status/corpo e mantém só um fato. |
| Resposta final de negócio 4xx | `http_idempotency_replays_final_business_4xx_response` | Reproduz o 404 original com `Idempotency-Replayed: true`; erros abaixo de 500 são resultados finais. |
| Requisições HTTP concorrentes | `http_idempotency_concurrent_same_key_appends_one_fact` | Um único fato, com segunda resposta bloqueada ou replay. |
| Falha pós-append e pré-resposta | `http_idempotency_failure_after_append_marks_unknown_without_duplicate_fact` | A repetição retorna `idempotency_outcome_unknown` e não duplica o fato. |
| Mesmo ID lógico, payload diferente | `http_idempotency_rejects_same_key_with_different_request` | Retorna `409 idempotency_key_reused`. |
| Mesmo JSON semântico com ordem de propriedades diferente | `http_idempotency_replays_same_semantic_json_with_reordered_keys` | Replay da mesma resposta e um único fato, mesmo que as chaves JSON estejam reordenadas. |
| Chave inválida/duplicada e request maior que 2 MiB | `http_idempotency_rejects_invalid_key_and_oversized_request_before_command` | `400`/`413` antes de executar o comando; nenhum fato persistido. |
| Path, query string ou Content-Type divergente | `http_idempotency_fingerprint_includes_path_query_and_content_type` | Reutilização da chave com fingerprint diferente é recusada, sem segundo append. |
| Resposta maior que 4 MiB | `oversized_response_marks_key_unknown_and_blocks_retry` | Resposta não cacheável leva a `unknown`; nova tentativa fica bloqueada. |

## Comandos

```bash
cargo test --lib ledger_state_machine_fails_closed_and_expires_only_completed_records -- --nocapture
cargo test --test acceptance_api http_idempotency -- --nocapture
cargo test --all-targets --all-features --locked
```

Todos os casos com banco exigem Docker/Testcontainers. O teste de trigger não equivale a um processo morto em qualquer instrução, failover ou partição de rede; testes operacionais específicos continuam necessários.
