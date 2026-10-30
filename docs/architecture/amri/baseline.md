# AMRI — baseline and implementation protection

| Baseline metadata | Value |
|---|---|
| Baseline date | 2026-10-30 |
| Repository | `israelsantiago/supplier-rfq-rust` |
| Reference branch | `main` |
| Reference commit | `553bbba203b7a7d62f5b2f06bf57f0342b914094` |
| Commit title | `docs(architecture): align RPU and Reactor boundaries` |

## 1. Remote repository state observed

| Item | Observation |
|---|---|
| Default branch | `main` |
| Remote branches | Only `main` was returned by the branch listing |
| Open pull requests | None returned |
| Repository archived | No |
| Latest successful Rust CI | Run #84, `41075206476` |
| CI head SHA | `553bbba203b7a7d62f5b2f06bf57f0342b914094` |

The branch/ref listing and CI metadata were read from GitHub on 2026-10-30. They describe the remote state at the time of inspection, not the user's local worktree.

## 2. Latest CI evidence

- **Run:** [Rust CI #84](https://github.com/israelsantiago/supplier-rfq-rust/actions/runs/41075206476)
- **Created:** `2026-10-30T14:50:28Z`
- **Completed:** `2026-10-30T14:52:23Z`
- **Conclusion:** `success`

| Workflow job | Result | Evidence scope |
|---|---|---|
| `test` | Success | `verify-rpu-boundaries.sh`, Rust formatting/checks/tests/Clippy included by that script, plus Testcontainers isolation verifier |
| `functional-core-branch-coverage` | Success | The explicit 100% branch-coverage gate for the Functional Core regions listed by the repository checker |
| `openapi-contract` | Success | OpenAPI lint |
| `mermaid-diagrams` | Success | Mermaid diagrams embedded in Markdown |
| `documentation-current-state` | Success | README/ADR index/current-state/link/branch-policy checks |
| `mutation-testing` | Skipped | The workflow runs this job only for `workflow_dispatch` or scheduled executions; it was not part of this push run |

A successful job proves only the steps it executed. The coverage result is not 100% coverage of the entire crate. The skipped mutation job must not be reported as passed.

## 3. Existing architecture to preserve

The current reference implementation contains:

- Independent command capabilities/RPUs: `OpenRfq`, `RegisterSupplier`, `SetSupplierEligibility`, `SubmitSupplierQuote` and `CloseRfq`.
- Query capabilities including `ListSupplierQuotes`, `GetRfqSummary` and `ListRfqSummaries`.
- `ApplicationStateProvider` and `FactstrApplicationStateProvider` for the legacy commands.
- `FinalizeRfqSourcing` with a specialized `AtomicSourcingStateProvider` and `FactstrAtomicSourcingStateProvider`.
- `FinalizeRfqSourcing` reads a composite context, runs one pure decision and submits four consequences through one `append_if`.
- `AtomicSourcingFlow` is a pass-through facade, not a multi-RPU Reactor.
- `scripts/verify-rpu-boundaries.sh`, `scripts/verify-application-state-boundary.sh`, `scripts/verify-functional-core-test-isolation.sh`, `scripts/verify-documentation-current-state.sh`, `scripts/verify-mermaid-diagrams.sh`, coverage and mutation runners.

The AMRI proposal does not alter these production paths in Stage 0/Stage 1. `FinalizeRfqSourcing` remains the single-capability atomic baseline and must remain available for regression comparison.

## 4. Local validation limitation

The repository checkout was not mounted in the execution environment at the time this baseline was prepared. The local path used by the maintainer is not accessible from this environment. Therefore these commands were **not rerun locally** as part of this baseline preparation:

```bash
cargo fmt --all -- --check
cargo check --all-targets --all-features --locked
cargo test --all-targets --all-features --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
bash scripts/verify-rpu-boundaries.sh
bash scripts/verify-application-state-boundary.sh
bash scripts/verify-functional-core-test-isolation.sh
bash scripts/coverage-functional-core.sh
bash scripts/mutate-functional-core.sh
bash scripts/verify-openapi.sh
sh scripts/verify-mermaid-diagrams.sh
bash scripts/verify-documentation-current-state.sh
```

This is deliberate: no local execution result is inferred from remote CI metadata. The generated documentation package includes an application script that checks the expected base SHA and a clean worktree before copying documents. The maintainer must run the documented validation gates after applying it locally.

## 5. Baseline protection rules

1. No existing RPU business rule, request/response contract, event name/payload, HTTP status or CCC query changes in the documentation-only proposal.
2. No direct write access is added to participant capabilities.
3. No modification to `FinalizeRfqSourcing` is part of ADR-0034 preparation.
4. No AMRI claim is marked implemented before the relevant code and automated evidence exist.
5. Mutation testing remains separately reported when skipped by ordinary push/PR CI.
6. Any future source change must be delivered incrementally, with focused tests and the full repository gates.
