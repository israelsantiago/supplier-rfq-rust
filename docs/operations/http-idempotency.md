# Operação — idempotência HTTP

## Contrato para clientes

Nos comandos `POST /api/rfqs`, `POST /api/suppliers`, `PUT /api/suppliers/{supplier_id}/eligibility`, `POST /api/rfqs/{rfq_id}/quotes`, `POST /api/rfqs/{rfq_id}/close` e `POST /api/rfqs/{rfq_id}/finalize-sourcing`, enviar `Idempotency-Key` com um identificador único por operação lógica. Em timeout/retry, reutilizar a mesma chave **e o mesmo payload/path/query**.

- Resposta original abaixo de 500: a mesma chamada é repetida com o status e o corpo originais; replay inclui `Idempotency-Replayed: true`.
- Mesma chave e request diferente: `409 idempotency_key_reused`.
- Chave ainda executando: `409 idempotency_in_progress`; esperar e repetir a mesma chave.
- Respostas finais (incluindo `409 command_context_changed`) são vinculadas à chave e reproduzidas. Mesmo quando o conflito CCC indica que o fato não foi gravado, uma nova tentativa de comando é uma nova execução HTTP e deve usar uma **nova `Idempotency-Key`**; repetir a chave antiga reproduz o `409` original.
- Resultado anterior incerto: `409 idempotency_outcome_unknown`. Não gerar chave nova sem investigar o resultado anterior.
- Sem header: compatibilidade com o comportamento anterior, sem garantia de idempotência HTTP.
- Respostas finalizadas são retidas por 24 horas. Depois desse prazo, uma chave concluída expirada poderá ser usada para nova execução.

## Reconciliação operacional

A tabela `http_idempotency_keys` é o ledger do protocolo HTTP; FACTSTR continua sendo a fonte autoritativa dos fatos de domínio. `in_progress` ou `unknown` não são eliminados automaticamente porque o processo pode ter gravado o fato e falhado antes de salvar a resposta. Antes de liberar uma chave, procure nos fatos autoritativos o resultado correspondente ao comando, correlacione horário/IDs/payload e registre a decisão operacional. Não delete nem reutilize essas chaves às cegas.

Consultar chaves pendentes usando a variável `DATABASE_URL` e `psql`:

```sql
SELECT operation_scope, idempotency_key, state, created_at, updated_at, expires_at
FROM http_idempotency_keys
WHERE state IN ('in_progress', 'unknown')
ORDER BY updated_at;
```

Para inspecionar uma chave específica:

```sql
SELECT operation_scope, idempotency_key, request_fingerprint, state,
       response_status, created_at, updated_at, expires_at
FROM http_idempotency_keys
WHERE operation_scope = 'POST /api/rfqs'
  AND idempotency_key = '<CHAVE>';
```

**Não existe limpeza automática para `in_progress`/`unknown`.** Como o Event Store FACTSTR e o ledger SQL não participam da mesma transação, o protocolo é conservador: bloqueia reexecução em caso incerto em vez de prometer exactly-once distribuído.

## Retenção e limites operacionais

O middleware aceita corpos de requisição de até 2 MiB quando há `Idempotency-Key` e armazena respostas de até 4 MiB. Um corpo maior é rejeitado antes da execução com `413`; uma resposta maior após o comando produz um resultado `unknown` para impedir reexecução automática. Clientes devem reutilizar a chave somente com o mesmo método, rota, caminho, query string, Content-Type e conteúdo JSON semântico.

Agende a limpeza de respostas `completed` expiradas a cada hora ou diariamente, de acordo com o volume. Exemplo:

```bash
DATABASE_URL="$DATABASE_URL" bash ~/Downloads/supplier-rfq-hardening/scripts/ops/cleanup_expired_idempotency.sh
```

O SQL remove apenas registros `completed` com `expires_at <= now()`. Nunca remova `in_progress` ou `unknown` por rotina de limpeza; esses registros exigem reconciliação com os fatos do FACTSTR.

## Pool e migration do ledger

Na composição de produção, `HttpIdempotencyStore::connect` cria pool limitado independente (máximo de quatro conexões) e valida/inicializa o ledger com checksum próprio sob advisory lock. O banco PostgreSQL pode ser o mesmo do FACTSTR/projeção, mas não há pool compartilhado. A migration raiz `migrations/202610270001_http_idempotency.sql` é preservada para compatibilidade histórica do migrator SQLx; a cópia em `migrations_idempotency/` mantém o mesmo DDL idempotente e é controlada por `http_idempotency_migrations`. Não altere o conteúdo aplicado sem uma nova versão de migration.

Para investigar, procure `in_progress` e `unknown` com a consulta acima e faça a reconciliação com os fatos antes de liberar uma chave. Não tente limpar esses estados com o job normal de expiração.
