# AMRI — protocol and proposed contracts

**Status:** Proposed; contracts are candidates for Stage 1 review.

## 1. Responsibilities

| Element | Owns | Must not own |
|---|---|---|
| Domain Capability | Domain language, local business rules, typed facts | Other capability internals |
| Autonomous RPU shell | Complete single-capability processing and its own state interaction | Multi-capability composition |
| Prepared capability participant | Invoking the same Functional Core and returning a non-persisted outcome | State writes or calls to another capability's shell |
| Composite Context Planner | Semantic union of required context and participant projections | Business decisions or store-specific query syntax |
| Atomic Interaction Reactor | Participant order, data hand-off, Provider/policy invocation, abort and commit request | Domain rules, raw Store/SQL access |
| Structural validator | Protocol and batch integrity | Cross-capability business semantics |
| Domain Composition Policy | Pure invariants spanning participants | I/O or persistence |
| Atomic Interaction State Boundary | Coherent snapshot and conditional batch commit | Domain decisions |
| Store adapter | Query translation, token binding, encoding and transaction/append mechanics | New business rules |

## 2. Two execution modes, one Functional Core

The established autonomous mode remains:

```text
request
  -> RPU Imperative Shell
  -> read capability context
  -> build typed context
  -> existing Functional Core
  -> proposed consequence(s)
  -> capability Application State Provider
  -> conditional write
  -> response
```

The opt-in AMRI participation mode is:

```text
interaction participant request
  -> PreparedCapabilityParticipant
  -> typed projection of shared snapshot
  -> same existing Functional Core
  -> PreparedOutcome + proposed fact(s)
  -> return to Reactor (no write)
```

`PreparedCapabilityParticipant` is an application-owned adapter/port, not a redefinition of the official RPU abstraction. The normal RPU remains complete. A capability may choose not to expose a prepared participation adapter if no atomic interaction requires it.

## 3. Candidate protocol types

These names define the vocabulary, not a mandate that every item becomes a public Rust struct. The first spike should keep the API small and statically typed.

| Type/concept | Responsibility |
|---|---|
| `InteractionId` | Stable identity of one logical interaction |
| `InteractionDefinitionId` + version | Identifies the declared participant set, dependencies and policies |
| `ParticipantId` | Stable identity of one participant within the interaction definition |
| `ExecutionContext` | Stable clock/IDs/correlation and request metadata captured once |
| `ContextRequirement` | Application-defined semantic requirement for state needed by a decision |
| `CompositeContextSpecification` | Canonical merged read-set description produced by the application planner |
| `SharedContextSnapshot` | One coherent snapshot with typed participant projections |
| `ConsistencyToken` | Opaque token bound by the adapter to that exact snapshot/read set |
| `PreparedOutcome` | Immutable result from one participant; includes its identity, facts and output needed by dependents |
| `ProposedFact` | Typed, not-yet-persisted consequence owned by a participant |
| `ExternalEvidence` | Provenance/version/time/expiry of an external observation or reservation |
| `AtomicCommitPlan` | Complete authorized facts, deterministic order, interaction identity, fingerprint and consistency token |
| `CommitReceipt` | Receipt for a completed atomic append |
| `CommitResolution` | Committed, already committed, conflict, definite failure or ambiguous outcome |

Avoid embedding `factstr::EventQuery`, `u64` store versions, `serde_json::Value`, `PgPool` or any domain fact in generic Core types.

## 4. Participant context requirements

Participants declare semantic requirements; they do not author backend query syntax. A Supply Chain application might declare:

| Participant | Required context |
|---|---|
| Close RFQ | RFQ open/closed lifecycle facts |
| Award supplier quote | RFQ lifecycle, full quote set, supplier registration and eligibility, prior award facts |
| Create purchase order | Selected quote, proposed/recorded award context, existing PO constraints and relevant purchase terms |

The application-specific planner combines requirements, removes only semantically safe duplicates and produces a canonical specification. The state adapter translates that specification into a single backend read plan and returns one snapshot/token.

If the application cannot produce one coherent snapshot for the union, it cannot start AMRI. It must not approximate a snapshot by collecting independent reads and combining their version numbers.

## 5. Dependencies between participants

Dependencies are declared in the interaction definition as a directed acyclic graph. A participant receives immutable predecessor outcomes through typed input adapters. It does not import another capability's private modules or invoke its autonomous RPU shell.

The first implementation uses a static participant graph and deterministic sequential topological order. It rejects cycles, duplicate participant IDs, missing required participants, undeclared predecessor data and an outcome from the wrong interaction definition/version.

Parallel execution is deferred. It can be considered only for independent participants after tests demonstrate that scheduling cannot alter the logical batch, error semantics, external evidence handling or commit order.

## 6. Validation is layered

1. **Local decision:** the participant's existing Functional Core decides its own business outcome.
2. **Protocol structure:** generic validator verifies participant set, ownership, dependencies, identity uniqueness, type/version allowlists, completeness and deterministic order.
3. **Composition semantics:** application-owned pure policies validate cross-capability invariants.
4. **External evidence:** domain policy verifies provenance, revision, expiry and required reservation semantics.
5. **State concurrency:** the atomic state boundary validates the original consistency token during conditional commit.

A Reactor sequences these steps; it must not implement the rules belonging to steps 1 or 3.

## 7. Candidate Rust boundaries

The first version should prefer a statically typed composition and explicit application ports. Exact associated types, object safety and outcome erasure must be settled with a compiling spike before a generalized registry is introduced.

Illustrative participant contract:

```rust
trait PreparedCapabilityParticipant {
    type Request;
    type ContextView;
    type PreviousOutcomes;
    type Outcome;

    fn prepare(
        &self,
        request: Self::Request,
        context: Self::ContextView,
        previous: Self::PreviousOutcomes,
        execution: &ExecutionContext,
    ) -> Result<Self::Outcome, ParticipantRejection>;
}
```

Illustrative state contract:

```rust
trait AtomicInteractionState {
    type Snapshot;
    type ConsistencyToken;

    fn read_composite_context(
        &self,
        specification: &CompositeContextSpecification,
    ) -> Result<SnapshotEnvelope<Self::Snapshot, Self::ConsistencyToken>, StateReadError>;

    fn commit_if_unchanged(
        &self,
        token: &Self::ConsistencyToken,
        plan: ValidatedAtomicCommitPlan,
    ) -> Result<CommitResolution, AtomicCommitError>;
}
```

The actual interfaces must avoid leaking backend types while still making the context/token binding impossible to accidentally discard or substitute. Private constructors or sealed validation states should prevent unvalidated outcomes from being converted into a commit plan.

## 8. Fact encoding and ownership

Participants return typed facts through application-owned outcome types. The Core does not require a universal JSON payload. A store adapter uses explicit codecs to encode authorized fact types and schema versions.

The structural validator confirms each participant only proposes fact contracts declared by its interaction definition. A domain policy confirms that the facts are mutually consistent. The adapter should receive an immutable validated plan, not arbitrary event names and payloads supplied by HTTP or external clients.

The adapter assigns or preserves the deterministic batch order from the interaction definition. Hash-map iteration and completion timing must not decide event order.

## 9. Error model

At minimum distinguish:

- `ParticipantRejected` — local domain rule rejected.
- `CompositionRejected` — cross-capability invariant failed.
- `ParticipantProtocolViolation` — participant identity, dependency or fact ownership invalid.
- `ExternalProviderUnavailable` — Provider could not supply required evidence.
- `ExternalEvidenceStale` — evidence no longer meets the policy's freshness/reservation requirement.
- `ContextChanged` — state changed after the snapshot.
- `InteractionIdempotencyMismatch` — same InteractionId used with a different semantic fingerprint.
- `AlreadyCommitted` — marker identifies a prior successful commit.
- `CommitFailed` — adapter can prove the append did not commit.
- `CommitOutcomeUnknown` — caller cannot determine whether commit occurred; recovery is mandatory before retry.

Do not map every error to a generic 409 or 500. Delivery adapters own protocol-specific mapping; the Core keeps typed categories.
