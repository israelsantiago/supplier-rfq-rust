# AMRI — pacote de continuidade do projeto

<!-- AMRI-CONTINUITY-CHECKPOINT-20261121:START -->

## Checkpoint vigente — 2026-11-21

- HEAD publicado de referência: `dc2bf33fbd6e95f0fdbaad77b3f8c644af794f93`.
- Guardrails zsh/Bash e runner obrigatório publicados em `engineering-learnings.md` e `engineering-guardrails.md`.
- Stage 5B.2: evidência final revisada — 17/17 gates PASS; 11 frescos, 6 históricos reutilizados após validação de 12 logs; mutation SKIPPED por política; CI remoto não confirmado.
- Stage 5B.3-R: este change set finaliza sincronização documental e handoff; a conclusão depende de publicação confirmada.
- Stage 5C: discovery gate; não implementar Reactor sem consumidor real na Application/Delivery.

Novo documento de entrada: [Handoff da Stage 5C](./stages/stage-05c-handoff.md).

<!-- AMRI-CONTINUITY-CHECKPOINT-20261121:END -->


**Versão documental:** 1.0\
**Data do checkpoint:** 2026-11-03\
**Branch de referência:** `feat/amri-stage4-three-prepared-participants`\
**Repositório:** [israelsantiago/supplier-rfq-rust](https://github.com/israelsantiago/supplier-rfq-rust)

Este diretório é o **ponto de retomada obrigatório** para continuar o trabalho AMRI em outro chat, máquina ou sessão. Ele complementa — não substitui — os documentos arquiteturais originais e os ADRs.

## Ordem de leitura para uma nova sessão

1. [Estado atual e próximo passo](./current-state.md)
2. [Arquitetura, responsabilidades e contratos](./architecture-and-boundaries.md)
3. [Registro de decisões e invariantes](./decision-register.md)
4. [Plano completo por fase](./stage-plan.md)
5. [Diretrizes e acordos de engenharia](./engineering-agreements.md)
6. [Qualidade, testes e evidências](./quality-and-evidence.md)
7. [Referências oficiais e fontes de verdade](./references.md)
8. [Mapa de fontes de verdade e catálogo ADR completo](./source-map-and-adr-catalogue.md)
9. [Prompt de transferência para novo chat](./chat-handoff.md)
10. [Histórico de execução](./stage-log.md)
11. [Registros detalhados de cada fase](./stages/)

## Fontes de verdade e precedência

Em caso de divergência, use esta ordem:

1. Código, testes e scripts existentes na branch remota apontada pelo checkpoint atual.
2. ADRs aceitos para decisões já tomadas (`docs/architecture/adr/`).
3. ADR-0034 e documentos AMRI (`docs/architecture/amri/`): propostas e contratos ainda sujeitos a evidência.
4. Este pacote: índice de continuidade, status por fase, acordos de trabalho e decisões conversacionais consolidadas.
5. Histórico de conversas: evidência de intenção e decisões discutidas, mas não prova de implementação.

Nunca classifique uma proposta como implementada apenas porque aparece neste pacote. Use os estados `Proposed`, `Implemented`, `Verified`, `Accepted` e `Reported by maintainer` com o significado descrito em [estado atual](./current-state.md).

## Política de atualização

Ao concluir qualquer fase, atualize no mesmo change set:

- o arquivo correspondente em `stages/`;
- `current-state.md` e `stage-log.md`;
- `stage-plan.md`, quando escopo ou gates mudarem;
- `decision-register.md` e ADR-0034, se uma decisão for refinada/aceita/rejeitada;
- `quality-and-evidence.md`, com comandos, contagens, resultados e limitações reais;
- referências, diagramas e índice, se necessário.

A cada nova fase ou retomada em chat, informe o estado qualitativo da janela de contexto. Não invente porcentagem: o ambiente não oferece ao assistente um medidor confiável de tokens restantes. Ver [acordo específico](./engineering-agreements.md#10-continuidade-e-janela-de-contexto).


## Guardrails transversais de engenharia

Leia [engineering-guardrails.md](./engineering-guardrails.md) antes de criar ou executar scripts de qualquer fase. O documento consolida regras de fail-closed, heredocs literais, backups, allowlists, diffs, manifests, checksums, pacotes verificáveis, isolamento de testes e publicação.

<!-- AMRI-STAGE5B2-READER-LINKS:START -->

## Leitura adicional da Stage 5B.2

- [Contratos externos read-only e WireMock-RS](../stage5b2-external-provider-contracts.md)

O contrato HTTP é provisório e exclusivo do test double; não representa APIs de fornecedores reais.
<!-- AMRI-STAGE5B2-READER-LINKS:END -->

<!-- AMRI-STAGE5B3R-README:START -->

## Checkpoint Stage 5B.2 / Stage 5B.3-R — 2026-11-20

A evidência final da Stage 5B.2 está publicada em
[`evidence/stage5b2-final-reverify-20261120T211047693659491Z`](https://github.com/israelsantiago/supplier-rfq-rust/tree/evidence/stage5b2-final-reverify-20261120T211047693659491Z):
17/17 gates PASS, zero FAIL. Mutation foi SKIPPED por política e a promoção
do status da fase não foi realizada. A conclusão formal continua sujeita
a revisão humana.

A próxima subetapa é 5B.3-R, documental. Os aprendizados operacionais estão
em [engineering-learnings.md](./engineering-learnings.md).
A Stage 5C deve integrar o primeiro consumidor real do Reactor.

<!-- AMRI-STAGE5B3R-README:END -->
