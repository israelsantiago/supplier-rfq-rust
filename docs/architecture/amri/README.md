# Atomic Multi-RPU Interaction (AMRI)

**Status:** Proposed architecture; the pure AMRI Core (Stage 2) and composite-context/atomic-state boundary (Stage 3) are implemented and evidenced. Reactor, prepared participants and the complete RFQ interaction remain future stages.

**Decision record:** [ADR-0034](../adr/0034-atomic-multi-rpu-interaction.md)

AMRI is an optional protocol for an interaction that must coordinate multiple independent capabilities, prepare their consequences without intermediate writes, validate their composition, and commit the authorized set atomically to one authoritative state store.

It is not a replacement for ordinary RPUs, ordinary Reactors, FACTSTR, or a general workflow engine. It is intended for bounded interactions with an explicit all-or-none business invariant.

## Documents

- [Baseline and current implementation protection](./baseline.md)
- [Protocol and proposed contracts](./protocol.md)
- [Consistency, CCC, idempotency and failure model](./consistency-model.md)
- [Incremental implementation and test plan](./implementation-plan.md)
- [Stage 3 — Composite Context and Atomic State Port](./stage-3-composite-context-state-port.md)
- [Applicability and generalization beyond Supply Chain](./applicability.md)
- [RFQ Supply Chain reference interaction](../../flows/atomic-rfq-sourcing-amri.md)

## Core architectural rule

The Core knows participant identity, execution identity, prepared outcomes, composition structure, commit plans and protocol errors. It does not know RFQ concepts, fact payload formats, EventQuery, SQL, FACTSTR, PostgreSQL, HTTP or any external vendor.

The application owns typed capabilities, participant adapters, context projections, composition policies and fact codecs. Infrastructure adapters own coherent snapshot and conditional commit mechanics.

## Status vocabulary

| Status | Meaning |
|---|---|
| Proposed | Design candidate; no implementation claim |
| Accepted | Decision reviewed and supported by the required evidence |
| Implemented | Code exists and relevant automated tests pass |
| Verified | Required automated and, where applicable, operational evidence was recorded |
| Superseded | A later ADR explicitly replaces the decision |

An ADR can be Accepted before every implementation stage is complete. Its implementation status must remain separate from its decision status.

<!-- AMRI-CONTINUITY-LINK:START -->
## Continuidade e handoff do projeto AMRI

O pacote versionado de estado atual, arquitetura, decisões, plano completo por fase,
acordos de engenharia, referências, evidências e retomada em novo chat está em
[docs/architecture/amri/continuity/README.md](./continuity/README.md).
<!-- AMRI-CONTINUITY-LINK:END -->
