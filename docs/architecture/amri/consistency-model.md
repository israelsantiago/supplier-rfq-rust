# AMRI — consistency, idempotency and failure model

**Status:** Proposed; this document defines required guarantees and candidate mechanisms.

## 1. The protected read set

For one atomic interaction, the protected context is the union of facts that can invalidate:

- any participant's local decision;
- any dependency passed from one participant to another;
- any cross-capability composition policy;
- any uniqueness or already-completed interaction condition.

The union must be explicit, canonical and versioned with the interaction definition. A missing dependency makes the design incorrect; an unnecessarily broad dependency can create avoidable conflicts.

## 2. One read, one snapshot, one token

The state boundary reads the complete composite specification once and returns:

- data needed to build all participant-specific typed views;
- a snapshot identity/metadata record;
- one opaque consistency token bound to the exact read set and backend context.

Never run a read per participant and combine the highest version numbers. `max(v1, v2, v3)` is not a snapshot protocol. The token must be returned by the same adapter that later validates it.

A read cursor or pagination cursor is not automatically a consistency token. The complete read set must be protected, including facts beyond any returned page if the rule depends on the full set.

## 3. FACTSTR reference semantics

For FACTSTR 0.5.2, the intended adapter behavior is:

1. Build one `EventQuery` from the complete composite specification.
2. Execute one query and retain its `current_context_version` as an opaque token.
3. Build all participant context views from that same returned event set.
4. Keep the exact same logical query definition for the write operation.
5. Encode the full validated fact batch and invoke exactly one `append_if(batch, query, expected_context_version)`.
6. Treat a conditional conflict as all-or-none failure and expose no successful participant commits.

The query semantics of the actual FACTSTR version are load-bearing. Filters combine using the documented/implemented OR semantics; within a filter, event types and payload predicates have their own OR semantics, with the type and payload match combined. Every proposed filter change requires focused tests for missing facts, matching facts, irrelevant facts and false-conflict boundaries.

The adapter must not replace the returned `current_context_version` with the highest sequence among returned records, a cursor value or a reconstructed number.

## 4. Other storage adapters

AMRI is not intrinsically FACTSTR-specific, but it does not make every transactional database automatically conformant.

A relational adapter must prove that the snapshot and commit protect the full read set, including predicate/phantom effects. Repeatable Read alone may not prevent write skew or phantoms. Depending on the model, the adapter may require Serializable isolation, explicit locks, version rows, advisory locks or constraints. It must keep one transaction for the final conditional validation and all writes, and integration tests must exercise opposing interleavings with independent connections.

A backend whose API cannot bind a commit to the original coherent snapshot must not advertise AMRI's all-or-none consistency guarantee.

## 5. External evidence is a separate consistency domain

A Provider response proves only what the Provider observed at a time and revision. FACTSTR's consistency token does not protect another system.

Evidence should contain provider identity, external reference, revision/ETag when available, checked timestamp, expiry and reservation/fencing token if supported. Domain policy decides whether the evidence is sufficient.

For exclusivity, require the external provider to reserve the resource or accept a conditional version. A lease needs expiry, renewal policy if applicable, owner/interaction identity, and release or compensation when the internal commit aborts. No database transaction may remain open while remote calls execute.

External irreversible actions should normally be dispatched after the authoritative commit using a durable intent/outbox fact and downstream idempotency. If an external action must occur before commit, its partial-failure/release protocol must be explicitly designed and tested. AMRI does not create a distributed transaction.

## 6. Idempotency and the interaction marker

### Proposed FACTSTR mechanism

The current FACTSTR `NewEvent` contract does not expose a general event metadata/idempotency field to the application. The reference design therefore proposes a technical fact in the same batch:

```text
amri_interaction_committed.v1
  interaction_id
  interaction_definition_id + version
  semantic_request_fingerprint
  commit_plan_fingerprint
  participant_ids + versions
  recovery references for the committed domain outcomes
  committed_at
```

The composite query includes the marker event filtered by `interaction_id` alongside all domain dependencies. This protects two attempts using the same interaction identity. The marker is appended together with every domain fact in one `append_if` call, so it cannot exist without its corresponding batch under the store's atomic append contract.

If a retry sees an existing marker:

- same semantic fingerprint: return an `AlreadyCommitted` receipt and invoke the domain recovery function;
- different fingerprint: reject as idempotency-key reuse;
- incomplete/unreadable marker: fail closed and require reconciliation; never append a second batch blindly.

A losing concurrent attempt should encounter a conditional append conflict because the winning batch changed the interaction-marker context.

### Response recovery

The generic Core cannot reconstruct a domain-specific HTTP response. It returns a receipt and recovery references. The domain application provides a recovery function/query that reconstructs the result from committed facts. The existing HTTP idempotency ledger can then be completed from that recovered result, but it is still not atomically committed with FACTSTR.

The marker payload, digest canonicalization, recovery references and projection behavior must be validated in Stage 3/Stage 7 before the mechanism is accepted.

## 7. Conflict and retry policy

Default: abort the interaction on `ContextChanged`. Do not treat prepared outcomes as committed. Do not retry only the final append using an old batch.

Optional bounded retry is permitted only for a declared retry-safe interaction. It must:

1. retain the same logical `InteractionId` and semantic request fingerprint;
2. preserve the execution metadata captured for that logical interaction where domain rules require stability;
3. read a new coherent composite snapshot;
4. rerun all participants whose decisions depend on the changed context;
5. rerun all affected Providers/policies as required by evidence expiry;
6. rebuild and revalidate the full plan;
7. make one new conditional commit attempt.

Human-reviewed supplier selections, approvals or other non-repeatable choices abort for explicit review rather than silently retrying against a different context.

## 8. Atomicity and ambiguous outcomes

The atomic store contract guarantees all-or-none behavior for one commit attempt. A network timeout after the database committed is a different problem: the client may not know whether the operation succeeded. The interaction marker and recovery protocol resolve this by querying the authoritative store before retrying.

An adapter must distinguish a definite rollback/failure from an ambiguous outcome. It must not return a generic failure that encourages an unsafe blind retry.

## 9. Required failure matrix

| Failure point | Expected durable state |
|---|---|
| Participant rejects | No AMRI batch |
| Participant panics/returns protocol error | No AMRI batch; fault recorded |
| Composition policy rejects | No AMRI batch |
| Provider unavailable | No AMRI batch unless the policy explicitly permits optional evidence |
| Evidence expired/invalid | No AMRI batch |
| Context changes before append | No AMRI batch |
| Fact codec fails before append | No AMRI batch |
| Conditional append definitely fails | No AMRI batch |
| Append commits, response is lost | Full batch including marker exists; retry resolves as AlreadyCommitted |
| Same InteractionId with different request | Reject; no second batch |
| External dispatch fails after commit | Internal batch remains committed; dispatcher retries/reconciles independently |

## 10. Performance constraints

- Never hold an open database transaction while calling remote Providers or waiting for human input.
- Keep participant preparation bounded and deterministic.
- Measure composite-query cost, number of facts read, batch size, append latency, lock contention and conflict rates.
- FACTSTR's current PostgreSQL append implementation uses exclusive locks on the event and append-batch tables; larger batches do not remove that physical serialization. Benchmark before making throughput claims.
- Any optimization of query planning, caching or projections must preserve the authoritative-context and CCC contract; read models are not substitutes for the protected write context unless their version protocol is formally proven.
