# Architecture decision — Event Sourcing with FACTSTR

Status: reference implementation maintained on the default branch `main`
Historical baseline: relational implementation preserved in Git history and backup refs

## Decision

Maintain the FACTSTR-backed append-only fact log in PostgreSQL as the authoritative store on `main`. The earlier relational implementation is a historical baseline retained in Git history and backup refs. The application does not write its own domain tables: FACTSTR owns the event-store schema and exposes `query`, `append`, and `append_if` through `factstr::EventStore`.

This is a reference implementation for one RFQ lifecycle, not a production procurement platform. `ListSupplierQuotes` still reconstructs quotes from facts; the `rfq_summary_projection` read model supports RFQ summary/list queries. A high-volume implementation must benchmark the projection and its consumer independently.

## Domain Capabilities / RPUs

| Capability / RPU | Intent | Context consistency boundary | Fact written |
|---|---|---|---|
| `OpenRfq` | Open an RFQ with fixed requested items | RFQ ID **or** RFQ reference | `rfq_opened.v1` |
| `RegisterSupplier` | Register supplier identity and its natural reference | Supplier ID **or** supplier reference | `supplier_registered.v1` |
| `SetSupplierEligibility` | Assign supplier eligibility | Supplier registration for supplier ID; prior eligibility history is intentionally excluded | `supplier_eligibility_changed.v1` |
| `SubmitSupplierQuote` | Validate and submit one supplier's quote for an RFQ | RFQ opened/closed state, supplier registration and eligibility, and existing quote for the RFQ/supplier pair | `supplier_quote_submitted.v1` |
| `CloseRfq` | Close an open RFQ | RFQ opened/closed state | `rfq_closed.v1` |
| `ListSupplierQuotes` | Read the quote view | Read only; no append | None |

Each command RPU owns its request, context interpretation, decision, and conditional append. The `Functional Core` receives explicit values and returns either a rejection or a fact to be recorded. The `Imperative Shell` uses FACTSTR to read and append.

## Facts and versioning

Fact type names are versioned (`*.v1`). Facts are immutable; corrections should be represented by new facts, not updates to old ones. The current implementation records:

- `rfq_opened.v1`: RFQ identity, currency and all required items; items are immutable in this reference flow.
- `supplier_registered.v1`: supplier identity, immutable natural reference and display name.
- `supplier_eligibility_changed.v1`: only the eligibility status change. The latest matching fact determines current eligibility.
- `supplier_quote_submitted.v1`: accepted quote, line prices, line totals, overall total, delivery time and validity.
- `rfq_closed.v1`: RFQ closure.

Currency and money are validated without floating-point arithmetic. Money is stored as integer minor units (centavos).

## Command Context Consistency (CCC)

The process is:

1. Build a `EventQuery` for all facts that can invalidate the command's decision.
2. Query the store and interpret the matching facts in sequence order.
3. Make the decision from that explicit context.
4. Call `append_if(facts, context_query, current_context_version)`.
5. If the command context changed, append nothing. Re-read once where helpful to return a current business rejection (e.g. duplicate quote or closed RFQ); otherwise return `command_context_changed`.

FACTSTR has one global, monotonically increasing sequence. `current_context_version` is the latest sequence number among facts matching the full context query; it is **not** the same as `last_returned_sequence_number` when a read cursor is applied. For FACTSTR 0.5.2, `EventQuery` combines filters with `OR`. Within one `EventFilter`, entries in `event_types` are alternatives (`OR`), and entries in `payload_predicates` are also alternatives (`OR`); the event-type match and payload match are combined with `AND`. In compact form: `OR(filters)`, with each filter evaluating `OR(event_types) AND OR(payload_predicates)`. This behavior is implemented by FACTSTR's PostgreSQL query matcher (`matches_query`/`matches_filter`; see [query_match.rs](https://github.com/ricofritzsche/factstr/blob/main/factstr-postgres/src/query_match.rs)). Therefore, query scope is business logic, not merely performance tuning.

### Contexts chosen by this implementation

- `OpenRfq`: `rfq_opened.v1` matching the requested `rfq_id` **or** the normalized `reference`. This provides an atomic CCC boundary for both identifier uniqueness and natural-reference uniqueness.
- `RegisterSupplier`: `supplier_registered.v1` matching the requested `supplier_id` **or** natural `reference`. This protects both identities atomically against duplicates.
- `SetSupplierEligibility`: the registration fact matching `supplier_id`. This example treats the operation as a state assignment, not a transition guarded by the previous status. Concurrent accepted assignments are ordered by committed fact order, and the last committed assignment defines the current status. If the domain requires transition preconditions, revise both the decision and its CCC context to include the current eligibility status. Supplier reference is immutable in this reference implementation; if the domain must allow reference changes, it needs an explicit capability/fact and defined historical uniqueness semantics.
- `SubmitSupplierQuote`: facts matching one of: the RFQ's opening fact, closure fact, registration and eligibility history for the given supplier, or an existing quote for that RFQ/supplier pair. A quote for supplier A does not alter supplier B's quote context.
- `CloseRfq`: the RFQ's opening/closure facts. A close fact invalidates a quote decision prepared against the open state.

Too-broad contexts create false conflicts; too-narrow contexts can allow invalid decisions. When adding business rules, update the RPU's context query and add a stale-context test.

## Storage/concurrency trade-offs

FACTSTR `0.5.2` implements its PostgreSQL append path by acquiring an exclusive table lock on `events` and `append_batches`, then evaluating the conditional context and appending the batch. This is a sound backend-level serialization mechanism for the current contract, but it can limit write throughput globally even when two commands have disjoint logical contexts. Logical CCC independence does not imply physically parallel writes in this backend implementation.

The application deliberately does not add a second homegrown lock/transaction protocol around FACTSTR. If throughput becomes a concern, benchmark end-to-end append latency and concurrency, then consider the store implementation's locking strategy rather than bypassing `append_if`.

## RFQ summary read projection

This branch also maintains an application-owned PostgreSQL read model in `rfq_summary_projection` (migration `migrations/202610240001_rfq_summary_projection.sql`). It is updated from a durable FACTSTR stream named `rfq-summary-projection-v1`, filtered to `rfq_opened.v1`, `rfq_closed.v1`, and `supplier_quote_submitted.v1`.

The event log remains authoritative. The projection is eventually consistent and serves `GET /api/rfqs/{rfq_id}/summary` and the paginated/filterable `GET /api/rfqs/summaries` endpoint. Commands still reconstruct their decision context from FACTSTR and call `append_if`; the projection is never used to authorize a command.

Each delivered FACTSTR batch is applied in a PostgreSQL transaction. `last_event_sequence` makes updates idempotent if a process crashes after committing the projection transaction but before FACTSTR persists its durable cursor. Projection rebuild must stop the application, truncate `rfq_summary_projection`, delete only the cursor `rfq-summary-projection-v1` from `durable_stream_cursors`, and restart; the authoritative `events` table must not be deleted.

## Tests and why they are structured this way

### Functional Core tests

Unit tests in `submit_supplier_quote/decision.rs` test calculations and rejection rules using explicitly constructed requests and contexts. They should not need PostgreSQL, HTTP or mocks.

### REST acceptance tests

`tests/acceptance_api.rs` launches a disposable PostgreSQL 16 container, connects FACTSTR to that database, starts Axum on an ephemeral port and sends HTTP requests. It verifies facts via FACTSTR queries rather than reaching into FACTSTR's private tables.

### Concurrency and CCC

`ccc_two_independent_stores_from_same_snapshot_commit_exactly_one_quote_fact` creates two distinct `PostgresStore` instances, which have independent worker threads/connections. Both RPUs read and prepare a quote from the same context version. A barrier releases their `append_if` calls only after the shared snapshot precondition has been asserted. Exactly one append succeeds; the stale conditional append conflicts; precisely one quote fact remains. This test is deterministic in the important sense that both writes carry the same expected context version; the barrier itself does not guarantee that their SQL statements begin at the exact same nanosecond.

`ccc_supplier_eligibility_change_invalidates_a_prepared_quote` prepares an eligible quote, appends a later blocked-eligibility fact through another store instance, and proves that the old quote cannot append. `ccc_independent_supplier_contexts_commit_without_false_conflict` proves that one supplier's quote does not logically invalidate another supplier's quote context. HTTP-level duplicate and close/submission tests cover status mapping and end-to-end behavior but are not the primary proof of database concurrency; a single `PostgresStore` instance queues operations on its own worker thread.

## Historical baseline comparison

The current default branch `main` contains the FACTSTR-backed implementation described here. The earlier relational implementation is retained in Git history and in the verified backup refs; no active comparison branch is required. To compare a preserved baseline, select its explicit commit or backup tag and inspect it against `main`; do not rely on a branch that has been deleted during repository consolidation.
## References

- [Rico Fritzsche — Request Processing Units](https://architecture.ricofritzsche.me/concepts/request-processing-units/)
- [Rico Fritzsche — Command Context Consistency](https://architecture.ricofritzsche.me/concepts/command-context-consistency/)
- [Rico Fritzsche — FACTSTR repository](https://github.com/ricofritzsche/factstr)
- [FACTSTR core concepts](https://github.com/ricofritzsche/factstr/blob/main/docs/core-concepts.md)
- [FACTSTR PostgreSQL append implementation](https://github.com/ricofritzsche/factstr/blob/main/factstr-postgres/src/postgres_store.rs)
- [FACTSTR Tool Rental example](https://github.com/ricofritzsche/factstr-tool-rental-example)



## Providers de tempo e identidade

A Imperative Shell obtém tempo e IDs por `ClockProvider` e `IdProvider`. O Functional Core recebe somente valores concretos. Em `SubmitSupplierQuote`, `quote_id` e `submitted_at` são escolhidos antes do laço de retries CCC e reaproveitados em todas as tentativas internas.

A semântica adotada é horário estável de submissão por execução lógica. Se um requisito fiscal/comercial exigir validação de validade no instante da aceitação, separar `submitted_at` de `decision_at`/`accepted_at`, mudar a decisão explicitamente e adicionar testes para a nova regra; essa alternativa não deve ser inferida incidentalmente do retry.

Decisões relacionadas: [catálogo de ADRs](architecture/adr/README.md), especialmente ADR-0010 e ADR-0011.
