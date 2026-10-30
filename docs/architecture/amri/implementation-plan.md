# AMRI — incremental implementation and test plan

**Status:** Stage 1 completed on 2026-10-30 as an architecture/design documentation baseline. No AMRI production implementation exists; Stages 2–9 remain pending.

Every stage is a separate reviewable delivery. A stage cannot be declared complete from compilation alone. Its code, tests, documentation, automated evidence and unresolved risks must be reviewed before the next stage begins.

## Stage 0 — Baseline and protection

**Goal:** Establish the exact starting point without changing production code.

**Work:** Record main SHA, remote refs, open PRs, latest CI run and job conclusions; inventory existing RPUs, state ports, `FinalizeRfqSourcing`, scripts and tests; record known limitations; preserve existing behavior.

**Documents:** `baseline.md`, architecture evidence matrix.

**Tests/evidence:** Use current remote CI evidence. Run the local full gates after the package is applied in the maintainer's checkout. Distinguish local test results, remote CI results and tests skipped by CI.

**Exit:** Reproducible baseline with explicit evidence provenance and no production code change.

## Stage 1 — ADR and protocol design

**Status:** COMPLETED — 2026-10-30 (architecture/design documentation baseline and validation only).

**Goal:** Resolve and document the proposed architecture contracts before implementing abstractions.

**Work completed:** Prepared ADR-0034 (AMRI-D01..D16), participant and Reactor boundaries, composite-context and opaque-token model, error taxonomy, deterministic ordering, proposed interaction marker/recovery approach, external-evidence semantics, applicability limits and the reference RFQ flow. The design explicitly preserves autonomous RPUs and `FinalizeRfqSourcing` as the existing single-capability atomic baseline.

**Documents:** ADR-0034, protocol, consistency model, applicability, baseline, implementation plan, reference flow and diagrams. The evidence matrix identifies future tests as targets, not tests that already exist.

**Tests/evidence recorded (2026-10-30):**

- `bash scripts/verify-documentation-current-state.sh` — PASS in the staging worktree and final checkout.
- `sh scripts/verify-mermaid-diagrams.sh` — PASS in the staging worktree and final checkout; all 12 Mermaid diagrams rendered by Mermaid CLI 11.
- Markdown relative links — PASS across 41 Markdown files.
- `git diff --check` — PASS.
- Decision-to-test traceability and proposed-versus-implemented status reviewed in ADR-0034 and `docs/testing/architecture-evidence.md`.
- No AMRI production tests or production implementation were created in this stage.

**Completion boundary:** Stage 1 is complete as a documented design baseline suitable for beginning the bounded Stage 2 Core spike. This does not mean that AMRI is implemented or that every proposed decision is accepted. ADR-0034 intentionally remains `Proposed`; AMRI-D01..D16 and backend-specific mechanisms remain subject to refinement and evidence from implementation stages. FACTSTR query/token semantics, marker/recovery behavior and adapter conformance must be proven in the relevant later stages.

**Exit:** Documentation and diagrams are coherent and validated, with no documentation-level ambiguity blocking the minimal Core spike. Stage 2 may begin against the proposed contracts, recording any changes through the ADR decision log. The acceptance of storage-specific atomicity and recovery guarantees remains gated by Stages 3 and 7.

## Stage 2 — Minimal AMRI Core

**Goal:** Implement domain/store-independent protocol primitives only.

**Work:** Add only justified types for interaction identity, participant identity/definition, execution context, prepared outcomes, proposed facts, validation state, commit plan and typed errors. Start with static composition, not a plugin registry or generic workflow engine.

**Tests:** Unit tests for identity/fingerprint rules, immutable outcomes, duplicate facts, allowed participants, dependency graph, cycle/missing participant, deterministic ordering, empty/incomplete plan and error classification. Add `proptest` for ordering, deduplication and composition invariants. No HTTP, database or Testcontainers.

**Gates:** fmt, check, lib tests, Clippy, AMRI Core branch coverage and mutation testing restricted to pure Core code.

**Exit:** Core tests pass without storage, delivery or Supply Chain dependencies.

## Stage 3 — Composite Context and Atomic State Port

**Goal:** Prove one coherent snapshot and one token for the union of participant requirements.

**Work:** Implement the application-owned context planner contract, opaque token, snapshot envelope and atomic state port. Build a fake adapter first, then the FACTSTR adapter. Validate query canonicalization and ensure read and write use the same logical query specification.

**Tests:** Requirement union/de-duplication, one read invocation, token-plan binding, provider/store mismatch, expired/wrong token, OR/AND EventQuery semantics, relevant-change conflict, irrelevant-change behavior, independent-store race and no version-max composition. Testcontainers is introduced only at the adapter integration layer.

**Exit:** FACTSTR adapter proves stale-token rejection and all-or-none commit against the actual dependency version.

## Stage 4 — Three prepared Supply Chain participants

**Goal:** Demonstrate at least three independent capability decisions without intermediate writes.

**Participants:** Close RFQ; Award Supplier Quote; Create Purchase Order. Each exposes an opt-in prepared adapter that calls the capability's shared Functional Core. Existing autonomous RPU contracts remain intact.

**Tests:** Full local decision matrices for each participant; autonomous/prepared parity for equivalent inputs; no-write assertions; ownership/type/version checks; predecessor outcome compatibility; failure of every participant independently. Existing `FinalizeRfqSourcing` remains as a comparison baseline.

**Exit:** Three participants produce typed outcomes and proposed facts with zero persistence and no cross-capability internal imports.

## Stage 5 — Reactor and external Providers

**Goal:** Coordinate participants in a deterministic interaction.

**Work:** Implement a static interaction definition and dependency DAG; sequential topological orchestration; stable InteractionId/ExecutionContext; fake ERP-reference and tax/compliance Providers; cancellation and timeout semantics. Provider calls occur outside store transactions.

**Tests:** Participant order and hand-off; abort on rejection; provider failure/timeout; expired evidence; request fingerprint mismatch; missing participant; cancellation; stable IDs/timestamps; no commit on any precommit failure. Test with Fakes, not live external services.

**Exit:** Reactor coordinates the interaction without containing domain rules or accessing raw storage.

## Stage 6 — Structural and domain composition validation

**Goal:** Prevent an individually valid set of outcomes from forming an invalid batch.

**Work:** Generic structural validator plus Supply Chain-owned pure composition policies; sealed immutable commit plan; deterministic ordering and fact ownership allowlists.

**Tests:** Cross-RFU references, mismatched supplier/quote/award/PO, currency and amount mismatch, item/quantity mismatch, dispatch referencing wrong PO, duplicate fact IDs, unauthorized event type, missing outcome and tampered plan fingerprint. Property tests explore combinations.

**Exit:** Invalid or incomplete compositions cannot produce a validated commit plan.

## Stage 7 — FACTSTR atomic commit, idempotency and recovery

**Goal:** Prove the primary all-or-none guarantee.

**Work:** Encode the complete plan in the adapter; implement one `append_if`; test the technical interaction marker in the same batch; implement `AlreadyCommitted` receipt and domain recovery. No participant invokes append.

**Tests:** Exactly one append call; one append batch includes all facts and marker; any rejection/context conflict stores zero facts; same interaction concurrent attempts produce one batch; same ID/different fingerprint rejects; ambiguous response recovery does not append twice; autonomous RPU versus AMRI race; deterministic interleavings with independent Store instances.

**Exit:** All concurrency, atomicity, marker and recovery tests pass with real PostgreSQL/Testcontainers.

## Stage 8 — Hardening and performance

**Goal:** Bound operational failure modes and quantify the cost.

**Work:** Structured tracing per interaction/participant; metrics and timeouts; bounded retries only for declared retry-safe interactions; fault injection; batch/read-set limits; runbook for ambiguous commit and external reservation cleanup; reproducible load tests.

**Tests:** Append timeout/connection loss, store unavailable, participant panic, Provider timeout, crash after commit, reservation expiry/release, large context and batch, sustained contention. Record p50/p95/p99, throughput, conflict rate and database lock behavior.

**Exit:** Operational limits and recovery procedures are evidenced; no unsupported production-readiness claim.

## Stage 9 — Generalization proof

**Goal:** Demonstrate that reusable primitives do not encode Supply Chain assumptions.

**Work:** Reuse the Core with a small, different domain such as resource reservation (availability, authorization, allocation). Keep only application-specific participants, context projections, policies and facts in that domain. Evaluate a second adapter only if its consistency semantics can be proven.

**Tests:** Core compile/test target with no RFQ, FACTSTR, PostgreSQL, SQLx or Axum dependencies; contract tests run for the second domain; no duplicated Core protocol; domain-specific behavior remains outside the Core.

**Exit:** Second-domain proof succeeds and extraction into a standalone crate is justified by demonstrated reuse, not anticipated reuse.

## Delivery rules for every stage

1. One focused change set per stage; no all-at-once refactoring script.
2. Update the ADR decision log and status when evidence changes a proposal.
3. Add automated tests with the implementation, not as a later cleanup.
4. Test pure logic without Testcontainers; use real PostgreSQL only for adapter/transaction/CCC behavior.
5. Preserve all existing tests and scripts; never weaken a guard to make a stage pass.
6. Run fmt, check, tests and Clippy; run documentation/Mermaid checks when docs change; run coverage and mutation for the designated pure modules.
7. Verify `git diff --check`, inspect the full diff and ensure no unrelated files changed.
8. Do not commit/push automatically unless the maintainer explicitly requests it.
9. Report exact commands, exit codes, test counts, CI run URLs and skipped jobs. Never infer a pass from an unexecuted command.
