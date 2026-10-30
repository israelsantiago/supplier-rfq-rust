# AMRI — applicability, reuse and extraction boundaries

**Status:** Proposed architecture guidance.

## 1. AMRI is optional

AMRI is not a default application architecture and does not replace a regular RPU or Reactor. Use it only when one bounded interaction needs contributions from multiple independent capabilities and there is a genuine all-or-none invariant over the persisted consequences.

| Situation | Preferred approach |
|---|---|
| One capability, one or many consequences | Autonomous RPU with its own state boundary |
| Multiple capabilities, independent commits are acceptable | Conventional Reactor coordinating complete RPUs |
| Multiple independent capabilities whose internal consequences must commit together | AMRI |
| Long-running human approval or external process | Durable process manager/workflow/saga, not one open AMRI snapshot |
| Atomicity required across independent databases/services | Only a proven distributed protocol; otherwise reservation/outbox/saga/reconciliation |

## 2. What should be reusable

The candidate extraction boundary is the protocol/Core, not the RFQ application:

- interaction and participant identities;
- participant definition and dependency graph mechanics;
- execution metadata contract;
- prepared outcome and proposed-fact envelope;
- structural validation;
- deterministic plan composition;
- consistency-token abstraction;
- atomic commit port contract;
- typed interaction error and receipt model;
- conformance test suite for adapters.

These primitives must not depend on a particular store, event format, domain vocabulary, serialization library, web framework or async runtime.

## 3. What remains in each application

- Domain capabilities and their Functional Cores.
- Participant adapters and typed request/context projections.
- Semantic merging of domain context requirements.
- Cross-capability Composition Policies.
- Fact schemas, event codecs and versioning policy.
- External Providers and evidence interpretation.
- Domain-specific recovery/response reconstruction.
- Storage adapter and its isolation/CCC implementation.
- Delivery adapters, auth and HTTP idempotency integration.

A shared library must not absorb these concerns merely to reduce lines of code.

## 4. Reuse across simple and complex domains

A simple domain can keep all capabilities as autonomous RPUs and never depend on AMRI. A domain with several independent capabilities may use a traditional Reactor when partial effects are acceptable. A domain with a strict atomic interaction can implement the prepared participant protocol and state adapter.

The second-domain proof should be small and deliberately unlike procurement. Resource reservation is a useful candidate: check availability, authorize the reservation and allocate the resource in one commit. It must reuse the same generic Core but provide its own typed context, participants, policy and facts.

## 5. Criteria before extracting a standalone crate

Do not publish/extract a crate merely because the directory is named `amri`. Extraction requires:

1. Core builds without RFQ/FACTSTR/PostgreSQL/SQLx/Axum dependencies.
2. At least two distinct domain applications reuse the same Core contracts.
3. At least one real adapter passes the full consistency and idempotency conformance suite.
4. The protocol is stable enough to version and document.
5. The crate API does not expose storage-specific tokens or payload formats.
6. Maintenance costs and compatibility policy are defined.

The initial reference implementation should remain feature-first and local to `supplier-rfq-rust` until these conditions are met.
