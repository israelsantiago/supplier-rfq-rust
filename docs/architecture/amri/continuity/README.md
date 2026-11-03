# AMRI — pacote de continuidade do projeto

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
