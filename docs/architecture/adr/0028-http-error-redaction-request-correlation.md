# ADR-0028: Respostas HTTP de erro interno e correlação de requisições

- **Status:** Accepted
- **Data:** 2026-10-29
- **Escopo:** Prioridade 1 de hardening; nenhuma regra de negócio é alterada.

## Contexto

Handlers convertiam erros internos de persistência/execução em respostas `500` contendo a mensagem técnica. Isso pode divulgar detalhes de conexão, schema ou infraestrutura. Também faltava um identificador uniforme devolvido ao cliente para correlacionar a falha com os logs.

## Decisão

- Respostas `500` mantêm `code = internal_error`, status e envelope JSON atuais, mas usam mensagem genérica estável.
- O erro original é registrado em evento de tracing; ele não é serializado no corpo HTTP.
- Middleware externo gera `X-Request-Id` UUID por requisição, devolve o cabeçalho também em respostas antecipadas e coloca o valor no span `http_request`.
- Erros de negócio 4xx, validações, regras de domínio, eventos e contratos de sucesso permanecem inalterados.

## Testes e critérios de aceite

- Teste unitário prova que mensagens contendo URI/credencial/schema não aparecem no body 500.
- Teste HTTP prova que resposta 500 recebe `X-Request-Id` válido.
- `cargo fmt`, todos os testes, `cargo check` e Clippy devem passar.

## Consequências e limites

O ID melhora correlação, mas não substitui autenticação, autorização, métricas, proteção contra dados sensíveis nos logs ou uma política de retenção. A origem do ID é a aplicação; o middleware não confia em valor fornecido pelo cliente.

## Referências

- [Rico Fritzsche — Functional Core / Imperative Shell](https://architecture.ricofritzsche.me/concepts/functional-core-imperative-shell/)
- [Rico Fritzsche — Request Processing Units](https://architecture.ricofritzsche.me/concepts/request-processing-units/)
