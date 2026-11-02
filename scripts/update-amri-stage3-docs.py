#!/usr/bin/env python3
"""Promote Stage 3 documentation only after the Stage 3 finalizer's gates pass."""
from __future__ import annotations

from pathlib import Path
import os

ROOT = Path(__file__).resolve().parents[1]


def replace_once_or_already(text: str, old: str, new: str, label: str) -> str:
    old_count = text.count(old)
    new_count = text.count(new)
    if old_count == 1 and new_count == 0:
        return text.replace(old, new, 1)
    if old_count == 0 and new_count == 1:
        return text
    raise RuntimeError(
        f"{label}: esperado exatamente um estado original ou atualizado "
        f"(original={old_count}, atualizado={new_count})"
    )


def write_all_atomically(files: dict[Path, str]) -> None:
    originals = {path: path.read_bytes() for path in files}
    staged: dict[Path, Path] = {}
    replaced: list[Path] = []
    try:
        for path, content in files.items():
            tmp = path.with_name(path.name + f".stage3-{os.getpid()}.tmp")
            if tmp.exists():
                raise RuntimeError(f"arquivo temporário já existe: {tmp}")
            staged[path] = tmp
            tmp.write_text(content, encoding="utf-8")
        for path, tmp in staged.items():
            tmp.replace(path)
            replaced.append(path)
    except Exception:
        for path in reversed(replaced):
            rollback = path.with_name(path.name + f".stage3-rollback-{os.getpid()}.tmp")
            rollback.write_bytes(originals[path])
            rollback.replace(path)
        raise
    finally:
        for tmp in staged.values():
            tmp.unlink(missing_ok=True)


def main() -> int:
    plan_path = ROOT / "docs/architecture/amri/implementation-plan.md"
    adr_path = ROOT / "docs/architecture/adr/0034-atomic-multi-rpu-interaction.md"
    readme_path = ROOT / "docs/architecture/amri/README.md"
    evidence_path = ROOT / "docs/testing/architecture-evidence.md"
    stage3_path = ROOT / "docs/architecture/amri/stage-3-composite-context-state-port.md"

    plan = plan_path.read_text(encoding="utf-8")
    plan = replace_once_or_already(
        plan,
        "**Status:** Stage 1 completed on 2026-10-30 as an architecture/design documentation baseline. No AMRI production implementation exists; Stages 2–9 remain pending.",
        "**Status:** Stages 1–3 completed as bounded, separately evidenced deliveries. Stage 2 provides the pure protocol Core; Stage 3 provides the application-owned composite-context and FACTSTR state-port proof. Stages 4–9 remain pending; ADR-0034 remains Proposed.",
        "implementation plan header",
    )
    stage_start = "## Stage 3 — Composite Context and Atomic State Port\n"
    stage_end = "\n## Stage 4 — Three prepared Supply Chain participants\n"
    if plan.count(stage_start) != 1 or plan.count(stage_end) != 1:
        raise RuntimeError("implementation plan: seção Stage 3 não é única")
    before, rest = plan.split(stage_start, 1)
    old_stage3, after = rest.split(stage_end, 1)
    completed_stage3 = """## Stage 3 — Composite Context and Atomic State Port

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
"""
    plan = before + completed_stage3 + stage_end + after

    adr = adr_path.read_text(encoding="utf-8")
    adr = replace_once_or_already(
        adr,
        "- **Status:** Proposed — design baseline prepared; implementation not started",
        "- **Status:** Proposed — Stage 2 pure Core and Stage 3 composite-context/state-port boundary implemented; full AMRI protocol remains unaccepted",
        "ADR-0034 status",
    )

    readme = readme_path.read_text(encoding="utf-8")
    readme = replace_once_or_already(
        readme,
        "**Status:** Proposed architecture; no AMRI production implementation exists yet.",
        "**Status:** Proposed architecture; the pure AMRI Core (Stage 2) and composite-context/atomic-state boundary (Stage 3) are implemented and evidenced. Reactor, prepared participants and the complete RFQ interaction remain future stages.",
        "AMRI README status",
    )
    link = "- [Stage 3 — Composite Context and Atomic State Port](./stage-3-composite-context-state-port.md)\n"
    if link not in readme:
        readme = replace_once_or_already(
            readme,
            "- [Incremental implementation and test plan](./implementation-plan.md)\n",
            "- [Incremental implementation and test plan](./implementation-plan.md)\n" + link,
            "AMRI README documents",
        )

    evidence = evidence_path.read_text(encoding="utf-8")
    evidence_heading = "## AMRI — Stage 3 composite context and Atomic State Port (2026-11-01)"
    if evidence_heading not in evidence:
        evidence += """

## AMRI — Stage 3 composite context and Atomic State Port (2026-11-01)

| Property | Evidence implemented | Scope / limit |
|---|---|---|
| Canonical union of participant read requirements | `application_state::atomic_interaction::context` unit/property-targeted tests | Application layer; does not add query types to the AMRI Core |
| FACTSTR EventQuery semantics and query shape | `factstr_adapter_proves_union_query_unchanged_token_and_atomic_conditional_batches` | FACTSTR 0.5.2/PostgreSQL; one filter per clause, one event type and one object payload selector per filter |
| One coherent snapshot and one token | `read_composite` returns one `SnapshotEnvelope`; query and current context version are retained in the private FACTSTR token | No independently read versions or `max(version)` composition |
| Token/store binding | Fake and FACTSTR adapter tests reject a token from a different logical store identity | Store identity is a stable configured logical-store key; FACTSTR token has no TTL |
| Stale/relevant context change | `factstr_adapter_proves_union_query_unchanged_token_and_atomic_conditional_batches` | PostgreSQL/Testcontainers integration; matching append invalidates prior token |
| Irrelevant context change | Same adapter integration test | Only proves changes outside the declared composite query do not advance that query's context version |
| Conditional batch all-or-none | Same adapter integration test | One `append_if`; no interaction marker/recovery protocol yet |
| Independent store race | Same adapter integration test using independent PostgresStore connections | At most one competing conditional append commits from the same snapshot |

The Stage 3 fake and coverage runners are separate from the PostgreSQL integration runner and do not start Testcontainers. Mutation testing is deferred to one consolidated campaign after the entire AMRI implementation; the scoped runner is not an intermediate gate. The integration gate intentionally starts Testcontainers. The ADR remains Proposed; marker, recovery, Reactor and RFQ participant behavior are not claimed as implemented.
"""

    stage3_doc = stage3_path.read_text(encoding="utf-8")
    stage3_doc = replace_once_or_already(
        stage3_doc,
        "**Status:** Planned package; implementation and evidence are not yet validated in the maintainer's checkout.",
        "**Status:** COMPLETED — gates and adapter integration evidence recorded locally on 2026-11-01; see implementation plan and architecture evidence matrix.",
        "Stage 3 document status",
    )

    # All files are computed and preconditions checked before the first write.
    write_all_atomically({
        plan_path: plan,
        adr_path: adr,
        readme_path: readme,
        evidence_path: evidence,
        stage3_path: stage3_doc,
    })
    print("PASS: implementation plan, ADR-0034, AMRI README, evidence matrix and Stage 3 document updated.")
    print("ADR-0034 remains Proposed; Stage 3 evidence does not claim Reactor or end-to-end RFQ atomicity.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
