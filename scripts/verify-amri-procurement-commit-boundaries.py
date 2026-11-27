#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
from pathlib import Path

FORBIDDEN_INFRA = re.compile(r"\b(?:factstr|sqlx|axum|EventStore|AtomicInteractionStatePort|NewAtomicFact|append_if|append_if_unchanged)\b")


def code_only(source: str) -> str:
    # Enough to avoid comments/doc comments and ordinary strings causing false positives.
    source = re.sub(r"(?s)/\*.*?\*/", " ", source)
    source = re.sub(r"(?m)//.*$", " ", source)
    source = re.sub(r'br#*".*?"#*', ' ', source, flags=re.S)
    source = re.sub(r'r#*".*?"#*', ' ', source, flags=re.S)
    source = re.sub(r'b?"(?:\\.|[^"\\])*"', ' ', source, flags=re.S)
    return source


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, required=True)
    root = parser.parse_args().repo.resolve()

    required = [
        "src/domain/procurement_budget.rs",
        "src/domain/purchase_order.rs",
        "src/amri_participants/procurement_commit.rs",
        "src/app/amri/procurement_commit_context.rs",
        "src/app/amri/procurement_commit_execution.rs",
        "src/app/amri/procurement_commit_reactor.rs",
        "src/events/mod.rs",
    ]
    for relative in required:
        path = root / relative
        if not path.is_file() or path.stat().st_size == 0:
            raise SystemExit(f"FAIL: source ausente/vazio: {relative}")

    pure_files = [
        root / "src/domain/procurement_budget.rs",
        root / "src/domain/purchase_order.rs",
        root / "src/amri_participants/procurement_commit.rs",
        root / "src/app/amri/procurement_commit_reactor.rs",
    ]
    for path in pure_files:
        code = code_only(path.read_text(encoding="utf-8"))
        if FORBIDDEN_INFRA.search(code):
            raise SystemExit(f"FAIL: infraestrutura/write API vazou para o Functional Core/Prepared Participant/Reactor: {path.relative_to(root)}")

    reactor_source = (root / "src/app/amri/procurement_commit_reactor.rs").read_text(encoding="utf-8")
    reactor = code_only(reactor_source.split("#[cfg(test)]", 1)[0])
    if "CompositeContextReader" not in reactor or reactor.count("read_composite") != 1:
        raise SystemExit("FAIL: Reactor deve depender somente de CompositeContextReader e executar exatamente uma leitura")
    if "ClockProvider" in reactor or "IdProvider" in reactor or "Provider" in reactor:
        raise SystemExit("FAIL: Reactor não pode capturar providers nem consultar providers externos")
    if re.search(r"\.(?:append|commit|append_if|append_if_unchanged)\s*\(", reactor):
        raise SystemExit("FAIL: Reactor não pode implementar commit/append")

    events = (root / "src/events/mod.rs").read_text(encoding="utf-8")
    for token in (
        '"procurement_budget_allocated.v1"',
        '"procurement_budget_reserved.v1"',
        '"purchase_order_created.v2"',
        "pub struct ProcurementBudgetAllocatedFact",
        "pub struct ProcurementBudgetReservedFact",
        "pub struct PurchaseOrderCreatedV2Fact",
    ):
        if token not in events:
            raise SystemExit(f"FAIL: contrato de evento v2/orçamento incompleto: {token}")

    # The v1 PO event must retain its field set and the patch must not rewrite the legacy
    # FinalizeRfqSourcing module or Delivery handlers.
    match = re.search(r"pub struct PurchaseOrderCreatedFact\s*\{(?P<body>.*?)\n\}", events, re.S)
    if not match:
        raise SystemExit("FAIL: PurchaseOrderCreatedFact v1 ausente")
    fields = re.findall(r"(?m)^\s*pub\s+(\w+)\s*:", match.group("body"))
    expected = [
        "purchase_order_id", "rfq_id", "supplier_id", "source_quote_id", "currency",
        "total_amount_cents", "delivery_days", "created_at", "items",
    ]
    if fields != expected:
        raise SystemExit("FAIL: PurchaseOrderCreatedFact v1 teve seu contrato alterado")

    if "procurement_budget_allocated.v1" not in (root / "docs/architecture/amri/continuity/stages/stage-05-reactor-and-providers.md").read_text(encoding="utf-8"):
        raise SystemExit("FAIL: Stage 5 documentation must identify budget allocation as a proposed contract")
    stage5_text = (root / "docs/architecture/amri/continuity/stages/stage-05-reactor-and-providers.md").read_text(encoding="utf-8")
    contract_text = (root / "docs/architecture/amri/continuity/procurement-budget-contract.md").read_text(encoding="utf-8")
    if "owner/produtor não está identificado" not in stage5_text:
        raise SystemExit("FAIL: Stage 5 documentation must keep budget ownership explicitly unresolved")
    if "release parcial" not in contract_text or "restituição" not in contract_text or "Stage 7 não pode persistir" not in contract_text:
        raise SystemExit("FAIL: budget contract must block writes until owner/lifecycle approval")
    readme_text = (root / "docs/architecture/amri/README.md").read_text(encoding="utf-8")
    handoff_text = (root / "docs/architecture/amri/continuity/chat-handoff.md").read_text(encoding="utf-8")
    state_text = (root / "docs/architecture/amri/continuity/current-state.md").read_text(encoding="utf-8")
    plan_text = (root / "docs/architecture/amri/continuity/stage-plan.md").read_text(encoding="utf-8")
    flow_text = (root / "docs/flows/atomic-rfq-sourcing-amri.md").read_text(encoding="utf-8")
    if "POST_APPLY_TEST=PASS" not in readme_text or "PRECHECK=PASS" not in readme_text:
        raise SystemExit("FAIL: README checkpoint does not reflect the v1.0.3 execution receipts")
    if "POST_APPLY_TEST=PASS" not in handoff_text or "Stage 6" not in handoff_text or "Stage 7" not in handoff_text:
        raise SystemExit("FAIL: chat handoff is stale or omits the Stage 6/7 boundary")
    if "post-apply test PASS" not in state_text or "Stage 6" not in state_text or "Stage 7" not in state_text:
        raise SystemExit("FAIL: current-state checkpoint is stale")
    if "Stage 5C-R" not in plan_text or "Stage 6" not in plan_text or "Stage 7" not in plan_text:
        raise SystemExit("FAIL: stage plan does not describe the corrective gate and next stage")
    if "procurement_budget_allocated.v1" not in flow_text or "owner/produtor" not in flow_text:
        raise SystemExit("FAIL: Procurement Commit flow omits the budget ownership gate")
    reactor_text = (root / "src/app/amri/procurement_commit_reactor.rs").read_text(encoding="utf-8")
    if "snapshot_fingerprint" in reactor_text or "context_specification_fingerprint" not in reactor_text:
        raise SystemExit("FAIL: fingerprint must be named for the context specification")
    budget_text = (root / "src/domain/procurement_budget.rs").read_text(encoding="utf-8")
    if "rejects_currency_mismatch_and_historical_duplicate_even_for_another_supplier" not in budget_text:
        raise SystemExit("FAIL: budget must test historical uniqueness across suppliers")
    for test_name in (
        "existing_po_v2_marks_the_rfq_as_finalized",
        "malformed_po_v2_payload_is_rejected_including_unknown_fields",
        "po_v2_event_for_another_rfq_is_rejected",
    ):
        if test_name not in (root / "src/app/amri/procurement_commit_context.rs").read_text(encoding="utf-8"):
            raise SystemExit(f"FAIL: required PO v2 projection test is missing: {test_name}")

    if "FinalizeRfqSourcing" not in (root / "docs/architecture/amri/README.md").read_text(encoding="utf-8"):
        raise SystemExit("FAIL: AMRI README must preserve the ADR-0031 legacy path")
    decisions = (root / "docs/architecture/amri/continuity/decision-register.md").read_text(encoding="utf-8")
    for token in ("D-5C-03", "D-5C-05", "D-5C-09", "D-5C-10", "D-5C-12", "D-5C-13", "D-5C-14", "ADR-0034 permanece `Proposed`"):
        if token not in decisions:
            raise SystemExit(f"FAIL: Stage 5 decision checkpoint is missing: {token}")

    # The runner enforces the exact repository path allowlist; this source-level guard
    # focuses on architectural invariants and remains runnable in minimal fixtures.

    print("PASS: Core/participants/Reactor have no storage, HTTP or write-port dependency.")
    print("PASS: Reactor uses one read-only snapshot read and returns prepared consequences only.")
    print("PASS: PO v1 contract unchanged; Budget Allocation and PO v2 are explicit new schemas.")
    print("PASS: ADR-0031/legacy Delivery paths remain outside this change set.")
    print("PASS: proposed budget ownership is documented; no production approval is implied.")


if __name__ == "__main__":
    main()
