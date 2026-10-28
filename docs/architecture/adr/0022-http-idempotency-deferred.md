# ADR-0022: Idempotência de comandos HTTP

- **Status:** Accepted
- **Branch de referência:** `main`

## Contexto

Uma resposta pode se perder depois que um comando confirmou um fato no FACTSTR Event Store. Sem um protocolo de idempotência na fronteira HTTP, uma nova chamada pode executar uma segunda vez a operação lógica. CCC protege a validade da decisão contra mudanças no contexto; não deduplica, por si só, requisições HTTP repetidas.

## Decisão

Os endpoints de comando aceitam uma chave opcional `Idempotency-Key` (1–200 caracteres ASCII visíveis). Para que a garantia seja aplicada, o cliente deve reutilizar a mesma chave ao repetir a mesma operação lógica.

O escopo da chave é o método HTTP e o template da rota. A impressão digital SHA-256 é calculada sobre método, rota, caminho, query string, Content-Type e corpo JSON normalizado (ou bytes em hexadecimal se o corpo não for JSON válido); o ledger armazena somente o digest, não o payload original. Reutilizar a mesma chave com outra impressão digital retorna `409 idempotency_key_reused`.

A primeira chamada registra a chave como `in_progress` antes de executar o comando. Outra chamada simultânea recebe `409 idempotency_in_progress`. Respostas finais abaixo de 500, inclusive rejeições de negócio 4xx, são armazenadas por 24 horas com status, headers de representação permitidos e bytes do corpo; uma repetição compatível reproduz a resposta e inclui `Idempotency-Replayed: true`. Respostas 5xx e falhas de persistência da resposta movem a chave para `unknown` quando possível; uma repetição recebe `409 idempotency_outcome_unknown` e não executa novamente o comando.

Todas as respostas finais abaixo de 500, inclusive `409 command_context_changed`, são armazenadas como concluídas e reproduzidas para a mesma chave. Embora esse conflito indique que o append do comando não foi realizado, repetir a mesma chave deve reproduzir a resposta original; uma nova tentativa após esse conflito representa uma nova execução HTTP e deve usar uma nova chave.

As entradas `in_progress` e `unknown` não expiram automaticamente. Exigem reconciliação operacional do ledger com os fatos autoritativos antes de liberar a chave. Chaves concluídas expiradas podem ser removidas e, após 24 horas, a mesma chave poderá iniciar outra execução.

## Consequências e limites

- Sem `Idempotency-Key`, o endpoint mantém o comportamento anterior; não há garantia contra repetição HTTP.
- O ledger usa PostgreSQL e a gravação de resposta é separada do append no FACTSTR. Não existe transação atômica que inclua os dois componentes. Um processo pode cair depois do append no FACTSTR e antes de marcar a resposta como concluída. O estado `in_progress`/`unknown` bloqueia a repetição automática, mas requer investigação e reconciliação; não promete exactly-once distribuído.
- O escopo de segurança é global por rota enquanto autenticação/tenant estiver adiado. Se autenticação e tenancy forem introduzidos, o principal autenticado/tenant deverá fazer parte do escopo.
- Rejeições 4xx são armazenadas como resultado da execução lógica. Para corrigir a requisição ou tentar uma operação lógica nova, usar uma nova chave.
- Isso é independente de CCC, da estabilidade de IDs/timestamps durante retries internos e da idempotência do consumidor de projeção.
