# AMRI — incremental implementation and test plan

**Status:** Stages 1–3 and implementation code for Stages 4A–4B are published as bounded deliveries. Stage 4B code is present in commit `5d278958855e41277bd00573d2cf419a50ee875d`; this checkpoint does not claim local Rust/Cargo/Clippy verification because the associated logs are not part of the published commit and no GitHub CI run was found for that SHA. Stages 5–9 remain pending; ADR-0034 remains Proposed.

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

**Status:** COMPLETED — 2026-10-30 (domain/store-independent protocol primitives and pure evidence only).

**Goal:** Implement the smallest justified set of reusable protocol primitives without introducing application, storage, delivery or workflow orchestration dependencies.

**Work completed:** Added `src/amri` with typed interaction/definition/participant/fact identities, opaque 256-bit fingerprints, retry identity binding, captured execution metadata, participant definitions (including explicit opt-in for a participant to return no facts), deterministic stable topological ordering, prepared outcomes, typed proposed facts, structural validation and a sealed `ValidatedCommitPlan`. The application supplies canonical request/plan fingerprints; AMRI Core does not choose serialization or hashing rules. The module is compiled independently with `rustc` and has no third-party production imports.

**Explicitly out of scope:** No composite snapshot, consistency token, Context Planner, Provider, Atomic Interaction State Port, FACTSTR adapter, Reactor, HTTP endpoint, RFQ participant adapter, persistence, Testcontainers or database integration was added. Those concerns remain in Stages 3–7. The validated plan is not yet an authorization to persist.

**Tests/evidence recorded (2026-10-30):**

- `bash scripts/verify-amri-core-boundaries.sh` — verifies the dependency boundary and standalone `rustc` compilation.
- `cargo test --lib --no-default-features --locked amri:: -- --test-threads=1` — unit and `proptest` cases only; the AMRI Core test filter excludes integration tests.
- `cargo check --lib --no-default-features --locked` and `cargo clippy --lib --no-default-features --locked -- -D warnings` — library checks without enabling application test-support features.
- `bash scripts/coverage-amri-core.sh` — requires 100% production code regions and both outcomes of each reported branch in the six AMRI Core production modules.
- `bash scripts/mutate-amri-core.sh` — `cargo-mutants` targets only the six pure production modules and executes only `amri::` library tests. The runner sets an invalid `DOCKER_HOST`; it does not run integration tests or Testcontainers.
- `bash scripts/verify-rpu-boundaries.sh` — full repository Rust regression gate, including the existing application tests.
- `bash scripts/verify-application-state-boundary.sh`, `bash scripts/verify-functional-core-test-isolation.sh`, `bash scripts/verify-openapi.sh`, `sh scripts/verify-mermaid-diagrams.sh` and `bash scripts/verify-documentation-current-state.sh` — existing repository architecture/documentation gates.

**Completion boundary:** Stage 2 is complete only for the pure protocol Core. It does not accept every AMRI decision, establish storage consistency, prove atomic persistence, create a Reactor or implement AMRI in the RFQ flow. ADR-0034 remains `Proposed`; the Stage 3 composite-context/state-port spike must validate backend semantics before any storage guarantee is claimed.

**Exit:** Core primitives compile independently; unit/property tests, 100% scoped branch/region coverage and targeted mutation testing pass without Docker/Testcontainers; the existing repository regression and architecture gates remain green. Stage 3 may begin with the snapshot/context contract and state-port spike.

## Stage 3 — Composite Context and Atomic State Port

**Status:** COMPLETED — composite context planner, application-owned state port and FACTSTR 0.5.2 adapter contract verified on 2026-11-01.

**Goal:** Prove one coherent snapshot and one token for the union of participant requirements.

**Work completed:** Added application-owned `ContextRequirement`/`CompositeContextSpecification` with deterministic JSON canonicalization, requirement union/de-duplication and a versioned SHA-256 specification fingerprint. Added `SnapshotEnvelope`, opaque adapter token, `AtomicInteractionStatePort`, a test-only fake adapter and a FACTSTR adapter under `src/application_state/atomic_interaction`. The Stage 2 `src/amri` Core remains unchanged and dependency-free. The FACTSTR token retains the exact EventQuery and `current_context_version` obtained by the one composite read; `append_if_unchanged` uses those exact values in one `append_if` call.

**Semantic decisions evidenced:** FACTSTR filters are OR alternatives; event type and one object payload predicate within each filter are AND constraints; multiple payload predicates would be OR and are therefore not used to encode a multi-field conjunction. The context version is the full matching-context version, not the last returned sequence. FACTSTR tokens have no TTL; relevant context changes make them stale. `StoreIdentity` binds a token to a configured logical store identity and allows independent connections to the same store to race correctly.

**Tests/evidence recorded:**

- `bash scripts/verify-amri-stage3-context.sh` — PASS: pure planner/fake tests, formatting, library check and Clippy; fake tests run with an invalid `DOCKER_HOST` and do not start Testcontainers.
- `bash scripts/coverage-amri-stage3-context.sh` — PASS: 100% production regions and both outcomes of every branch in `context.rs` and `state_port.rs`.
- Mutation testing — DEFERRED: the scoped runner is retained, but the consolidated campaign is reserved for completion of the entire AMRI implementation; no Stage 3 mutation pass is claimed.
- `bash scripts/verify-amri-stage3-factstr.sh` — PASS: FACTSTR 0.5.2/PostgreSQL Testcontainers proof for query OR/AND semantics, irrelevant changes, stale token, all-or-none batch and independent-store race.
- Existing full repository, Application State, Functional Core/Testcontainers isolation, OpenAPI, Mermaid, AMRI Core boundary and documentation gates — PASS.

**Explicitly out of scope:** No Reactor, prepared RFQ participants, domain composition policy, interaction marker, idempotent recovery, HTTP endpoint or final multi-RPU RFQ atomic interaction was added. ADR-0034 remains Proposed.

**Exit:** One canonical composite query/read produces one snapshot and one opaque token; the adapter reuses the exact query/version for one conditional append; relevant changes reject the full batch, irrelevant changes do not create false conflicts, and independent connections permit at most one competing append.

## Stage 4 — Three prepared Supply Chain participants

**Status:** Stage 4A Shared Functional Core and Stage 4B prepared-participant code are published. Stage 4B reference commit: `5d278958855e41277bd00573d2cf419a50ee875d`. Verification evidence must be recorded from the maintainer's local Rust gates; this documentation checkpoint does not infer a test pass from the existence of the commit.

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

<!-- AMRI-CONTINUITY-CHECKPOINT:START -->
## Checkpoint de continuidade — 2026-11-03

O status remoto, decisões consolidadas, acordos de engenharia, histórico e handoff para
novo chat são mantidos em [continuity/README.md](./continuity/README.md). O status de fase
mais recente está em [continuity/current-state.md](./continuity/current-state.md); a execução
por fase e os gates estão em [continuity/stage-plan.md](./continuity/stage-plan.md).

No checkpoint deste documento, Stage 4A está publicada no commit
`203589e4d2c23338c8040e7802fabc77203fd6a9`. Stage 4B V1 ainda não foi aplicada: o dry-run
foi abortado por validação rustfmt em temporários, sem alteração do repositório. Consulte o
registro de sessão e o arquivo `stages/stage-04b-prepared-participants.md` antes de retomar.
<!-- AMRI-CONTINUITY-CHECKPOINT:END -->
