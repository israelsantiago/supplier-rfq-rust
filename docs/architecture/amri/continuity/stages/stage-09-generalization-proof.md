# Stage 9 — Prova de generalização para outro domínio

**Estado:** Pendente; depende de Stage 8 ou de um conjunto mínimo de protocol contracts estáveis.

## Objetivo

Demonstrar que o Core AMRI não contém suposições de RFQ/Supply Chain e é reutilizável por um domínio com linguagem e regras diferentes.

## Domínio candidato

Uma interação simples de reserva de recurso pode ter participantes como:

- verificar disponibilidade;
- validar autorização;
- alocar/reservar o recurso.

A escolha final deve ser feita com base em cenário pequeno, claramente diferente de RFQ, sem integrar sistema externo real.

## Escopo

- Reusar `src/amri` sem adicionar tipos/constantes de RFQ.
- Criar context projections, facts, participants e policies exclusivamente no novo domínio.
- Reusar contrato de identidade, DAG, PreparedOutcome, structural validation e commit plan.
- Demonstrar adapter conformance separadamente; não afirmar que todo storage suporta AMRI.
- Compilar/testar Core sem RFQ, FACTSTR, PostgreSQL, SQLx ou Axum.

## Testes e análise

- Mesmas invariantes de complete participants, dependencies, ownership, fact ids, ordering e fingerprint.
- Policies novas ficam no domínio novo, não no Core.
- Compare complexity/benefit; identificar parâmetros que realmente são genéricos.
- Revisar se extração para crate separado reduz acoplamento/duplicação na prática.

## Não fazer

Não extrair crate apenas por antecipação, não construir registry dinâmico/type-erasure framework sem necessidade comprovada, não duplicar o protocolo em segundo domínio.

## Gates de saída

Um segundo domínio utiliza o Core sem alterações específicas de RFQ; documentação e conformance test suite demonstram portabilidade. Só então decidir se crate independente, versionamento e distribuição têm benefício real.
