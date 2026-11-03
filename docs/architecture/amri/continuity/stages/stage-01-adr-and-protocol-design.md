# Stage 1 — ADR e desenho do protocolo

**Estado:** Concluída como fase de design/documentação em 2026-10-30.\
**ADR:** `docs/architecture/adr/0034-atomic-multi-rpu-interaction.md` — continua `Proposed`.

## Objetivo

Resolver contratos do protocolo antes de adicionar runtime: conceitos, papéis, consistência, identidade, erros, dependências, external evidence, commit, idempotência, recuperação, aplicabilidade e critérios de aceitação.

## Decisões de desenho registradas

- AMRI opt-in e limitado a uma interação atômica curta.
- Preservar o RPU autônomo `process(request) -> response`.
- Participante preparado chama o mesmo Functional Core e não faz writes.
- Um planner junta requirements; uma leitura produz snapshot coerente e um único token opaco.
- Dependências explícitas; primeiro runtime será sequencial e topológico.
- Validação estrutural genérica separada de composição de domínio.
- External Providers fornecem evidência; não criam atomicidade distribuída.
- Um único limite de commit no store autoritativo; FACTSTR target é um append condicional.
- InteractionId + fingerprint; marker e recovery ainda exigem prova.
- Core independente de domínio, store e delivery.
- Conflito aborta e exige nova leitura/preparação; retries só com semântica explicitamente segura.
- Workflows longos/human approval não pertencem ao protocolo atômico inicial.

## Entregáveis existentes

`ADR-0034`, `docs/architecture/amri/{README,baseline,protocol,consistency-model,implementation-plan,applicability}.md`, fluxo e diagramas Mermaid.

## Evidência histórica

O plano original registra verificação documental, Mermaid e links. Isso prova coerência do design naquela data, não implementação do protocolo completo.

## Critério de saída

Contratos suficientes para iniciar Core mínimo sem criar prematuramente adapter, Reactor, participants ou storage. ADR-0034 permanece Proposed até os gates finais da Stage 7–9.
