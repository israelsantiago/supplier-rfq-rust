# AMRI Stage 3 — Composite Context and Atomic State Port

**Status:** COMPLETED — gates and adapter integration evidence recorded locally on 2026-11-01; see implementation plan and architecture evidence matrix.

**Base:** Stage 2 published branch `feat/amri-stage2-core`, commit `0061795bb63c5455d9abb8e3ea6da22db671fd26`.

**Delivery branch:** `feat/amri-stage3-composite-context`.

## 1. Objective and completion boundary

Prove that one AMRI interaction can declare the union of all participants' read requirements, obtain one coherent FACTSTR snapshot with one context version, and use the exact same logical query and version in one conditional batch append.

Stage 3 is deliberately split into two independently verifiable increments:

- **3A — Application contract and fake adapter:** canonical requirement union, application-owned snapshot/port types, opaque fake token, and deterministic unit tests. This increment does not require Docker or PostgreSQL.
- **3B — FACTSTR 0.5.2 adapter proof:** retain the query returned from the canonical specification inside the opaque token; issue one `query`; pass that exact query and `current_context_version` to one `append_if`; prove OR/AND matching, irrelevant versus relevant changes, all-or-none batch behavior and independent-connection races against PostgreSQL/Testcontainers.

Stage 3 does **not** implement a Reactor, prepared RFQ participants, cross-capability composition policies, an interaction marker, recovery, HTTP delivery, or the final Supply Chain interaction. Those remain subsequent stages.

## 2. Placement and dependency rules

The Stage 2 Core remains in `src/amri` and must not import application, FACTSTR, SQLx, PostgreSQL, JSON, or runtime dependencies.

Stage 3 application/adapter code is placed in `src/application_state/atomic_interaction` and exported by `src/application_state.rs`. This placement is intentional:

- `context.rs` owns the application-specific context requirement and canonical union; it contains no FACTSTR query types.
- `state_port.rs` exposes store-neutral snapshot, opaque-token and conditional-batch contracts.
- `fake.rs` is compiled only under `cfg(test)` and proves the application port without external infrastructure.
- `factstr.rs` is the only Stage 3 application module that imports FACTSTR types and translates the canonical specification into `EventQuery`.
- `tests/amri_factstr_state.rs` is an integration test outside `src/`; it is the only Stage 3 test that starts Testcontainers.

No new Cargo dependency is required. `factstr = 0.5.2`, `factstr-postgres = 0.5.2`, `serde_json`, `sha2`, `thiserror`, Tokio and Testcontainers are already declared in the Stage 2 baseline.

## 3. Context requirement model

A `ContextRequirement` is declared by one `ParticipantId` before the state read. It contains one or more `ContextClause` values. Each clause means:

`event_type == X AND payload is a recursive subset of object Y`.

The composite planner:

1. Rejects an empty requirement set.
2. Rejects duplicate participant declarations.
3. Rejects a requirement union with no clauses (an empty FACTSTR filter list would be dangerously broad).
4. Canonicalizes JSON object keys recursively.
5. Deduplicates identical `(event_type, canonical payload object)` clauses.
6. Sorts participant IDs and clauses deterministically.
7. Computes a versioned SHA-256 fingerprint over the canonical specification.
8. Leaves query construction to the storage adapter; the FACTSTR adapter maps one clause to one `EventFilter`.

A multi-field conjunction must be represented by **one JSON object predicate**, for example `{"rfq_id":"RFQ-1","tenant":"T1"}`. It must not be split into two `payload_predicates`, because FACTSTR treats multiple payload predicates as OR alternatives.

## 4. FACTSTR 0.5.2 semantics that constrain the adapter

The adapter must preserve these contract semantics:

- Multiple `EventQuery.filters` are OR alternatives.
- Multiple `event_types` inside one filter are OR alternatives.
- Multiple `payload_predicates` inside one filter are OR alternatives.
- Event-type matching and payload-predicate matching within one filter are AND constraints.
- A JSON object predicate matches recursively by subset; additional keys in the stored payload do not invalidate a match.
- Array predicates are order-insensitive subsets: every requested element must match an element in the stored array; additional stored elements are allowed. The PostgreSQL integration test covers an object subset inside an array.
- `min_sequence_number` limits returned rows only; it does not narrow the context version used by `append_if`.
- `current_context_version` represents the full matching context, while `last_returned_sequence_number` describes the returned rows. They must not be substituted for one another.
- A successful `append_if` commits one batch. A context conflict commits none of that batch.

The implementation therefore emits one EventFilter per canonical clause, gives each filter one event type and one object payload predicate, and leaves `min_sequence_number` unset. The adapter builds the EventQuery once during `read_composite` and retains that exact query in the private token. `append_if_unchanged` accepts only the token, not a second query argument, so callers cannot accidentally rebuild a different read set for the write.

References:

- [FACTSTR 0.5.2 EventQuery source](https://docs.rs/crate/factstr/0.5.2/source/src/event_query.rs)
- [FACTSTR 0.5.2 EventFilter source](https://docs.rs/crate/factstr/0.5.2/source/src/event_filter.rs)
- [FACTSTR 0.5.2 EventStore trait](https://docs.rs/crate/factstr/0.5.2/source/src/event_store.rs)
- [FACTSTR 0.5.2 QueryResult source](https://docs.rs/crate/factstr/0.5.2/source/src/query_result.rs)
- [Rico Fritzsche — Event Store API](https://architecture.ricofritzsche.me/specifications/command-context-consistency/event-store-api/)

## 5. Token rules

The FACTSTR token is the pair of:

- the `current_context_version` returned by the one composite query; and
- the exact query used to obtain that version.

The adapter wraps these values with the composite-specification fingerprint and a configured `StoreIdentity`. The identity is a stable application configuration name for one logical event store, not a transient connection/pool identity. Two independent pools for the same logical database may use the same identity; different logical stores must not.

**FACTSTR context tokens do not have a TTL.** They become stale when the matching context changes. “Expired token” is therefore not a valid Stage 3 FACTSTR invariant. The tests distinguish:

- stale token: a matching fact was appended after the snapshot;
- irrelevant append: a fact outside the query does not change the context version;
- wrong-store token: token from another configured logical store is rejected before append;
- wrong specification: structurally prevented by keeping the query private inside the token and not accepting a second query during append.

No `max(version_a, version_b)` or other composition of independently read versions is allowed. The only token is the one returned by the one composite read.

## 6. Atomic state port contract

`AtomicInteractionStatePort` exposes:

- `read_composite(specification) -> SnapshotEnvelope<Token>`;
- `append_if_unchanged(token, non_empty_fact_batch) -> CommitReceipt | ContextChanged | StoreError`.

The public port does not expose `EventQuery`, `EventFilter`, `NewEvent`, `EventStore`, `PostgresStore`, SQLx or a raw connection. Its snapshot uses application-owned read records (`sequence_number`, `event_type`, `payload`). Domain-specific typed projections/codecs remain owned by the application and are not universal AMRI Core facts.

The FACTSTR adapter maps the application's write batch to `NewEvent` and issues exactly one `EventStore::append_if` call. The technical interaction marker and idempotent recovery are not added in Stage 3; those are Stage 7 responsibilities.

## 7. Evidence matrix and required tests

### Stage 3A — pure/application boundary

| Property | Required evidence | Infrastructure |
|---|---|---|
| Requirement union and de-duplication | Order-independent canonical union and duplicate-clause tests | Unit/fake |
| One participant declaration | Duplicate participant requirement rejected | Unit/fake |
| Query-specification canonicalization | Stable fingerprint for reordered JSON keys, including nested objects/arrays | Unit |
| Single composite read | Fake adapter read counter equals one for one envelope | Fake |
| Relevant context change | Old token rejects the entire batch | Fake |
| Irrelevant context change | Old token remains valid if no matching fact changed | Fake |
| Store mismatch | Token from another store identity is rejected | Fake |
| No independent-version composition | API returns one snapshot envelope and one token; no API accepts/combines a list of versions | API/contract review |

### Stage 3B — FACTSTR/PostgreSQL adapter

| Property | Required evidence | Infrastructure |
|---|---|---|
| Exact OR/AND behavior | Positive/negative records and exact query filter shape; only intended subset matches | FACTSTR/PostgreSQL |
| Query mapping shape | One EventFilter per clause, one event type and one object predicate per filter | Adapter unit/integration |
| Read/write query binding | Adapter retains the original query in private token; append receives no second query | Code review + integration |
| Irrelevant change | Unmatched event does not invalidate token | PostgreSQL/Testcontainers |
| Relevant change | Matching event invalidates old token | PostgreSQL/Testcontainers |
| Atomic batch | Successful receipt contains all consecutive events | PostgreSQL/Testcontainers |
| Stale batch all-or-none | Conflict stores zero facts from attempted batch | PostgreSQL/Testcontainers |
| Independent-store race | Two independent connections using same logical store identity and same token; exactly one conditional append succeeds | PostgreSQL/Testcontainers |

The fake proves the port contract only; it does not substitute for the real FACTSTR adapter test. The PostgreSQL proof validates FACTSTR 0.5.2 behavior, not all possible database adapters.

## 8. Explicit non-goals

- No modification to `src/amri` production modules.
- No Reactor, participant execution, or Supply Chain capability adapter.
- No domain composition policy.
- No commit marker or idempotent response recovery.
- No external Provider or distributed transaction claim.
- No independent read/version merge or version-max heuristic.
- No full RFQ flow and no HTTP endpoint.
- No change to `Cargo.toml` or `Cargo.lock` is expected.

## 9. Exit criteria

Stage 3 may be marked complete only when:

1. Stage 3A contract/fake tests, Clippy and scoped 100% coverage/branch checks pass.
2. FACTSTR 0.5.2 PostgreSQL tests prove OR/AND semantics, token binding, stale-token rejection, irrelevant-change behavior, all-or-none batch append and independent-connection race.
3. The existing RPU, Application State, Functional Core isolation, OpenAPI, Mermaid and documentation gates remain green.
4. `src/amri` remains dependency-free and all tests under the Stage 3 fake runner execute without Docker/Testcontainers.
5. The diff contains only the explicit allowlist and is reviewed before commit/push.

**Mutation testing policy:** mutation testing is intentionally deferred during Stage 3 and all intermediate AMRI stages. The scoped runner is retained, but no mutation pass is claimed or required here. Execute one consolidated mutation campaign only after the complete AMRI implementation is finished, restricted to eligible pure Functional Core logic and without PostgreSQL/Testcontainers.

ADR-0034 remains **Proposed**. Stage 3 proves the composite read/token/port boundary only; it does not yet prove the final AMRI participant protocol, marker/recovery, or full RFQ all-or-none interaction.
