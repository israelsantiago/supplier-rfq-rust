#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"; cd "$ROOT"
python3 - <<'PY_GUARD'
from pathlib import Path
import re, sys
known = {
 "src/application_state/factstr.rs": ["application_state::factstr::tests::provider_reads_typed_facts_and_rejects_stale_conditional_append"],
 "src/capabilities/set_supplier_eligibility/load_context.rs": ["set_supplier_eligibility::load_context::context_scope_tests::eligibility_history_does_not_change_assignment_context_version"],
 "src/capabilities/submit_supplier_quote/process_request/ccc_tests.rs": [
   "submit_supplier_quote::process_request::ccc_tests::ccc_two_independent_stores_from_same_snapshot_commit_exactly_one_quote_fact",
   "submit_supplier_quote::process_request::ccc_tests::ccc_supplier_eligibility_change_invalidates_a_prepared_quote",
   "submit_supplier_quote::process_request::ccc_tests::ccc_independent_supplier_contexts_commit_without_false_conflict",
 ],
 "src/http/idempotency.rs": ["http::idempotency::tests::ledger_state_machine_fails_closed_and_expires_only_completed_records", "http::idempotency::tests::oversized_response_marks_key_unknown_and_blocks_retry"],
 "src/projections/rfq_summary_projection/mod.rs": ["projections::rfq_summary_projection::tests::replaying_the_same_fact_batch_does_not_double_count_quotes", "projections::rfq_summary_projection::tests::failed_projection_batch_rolls_back_earlier_events"],
}
marker = re.compile(r"Postgres::default\s*\(|ContainerAsync\s*<|testcontainers_modules|\bAsyncRunner\b")
found = {p.as_posix(): p.read_text(encoding="utf-8") for p in sorted(Path("src").rglob("*.rs")) if marker.search(p.read_text(encoding="utf-8"))}
if set(found) != set(known):
    print("FALHA: inventário de Testcontainers dentro de src/ mudou; revisar antes de ampliar filtros.", file=sys.stderr)
    print("Encontrados: " + ", ".join(sorted(found)), file=sys.stderr)
    print("Esperados: " + ", ".join(sorted(known)), file=sys.stderr)
    sys.exit(1)
for rel, names in known.items():
    source = found[rel]
    if "Postgres::default" not in source:
        raise SystemExit(f"FALHA: o arquivo listado não contém criação PostgreSQL: {rel}")
    if rel == "src/capabilities/submit_supplier_quote/process_request/ccc_tests.rs":
        parent = Path("src/capabilities/submit_supplier_quote/process_request.rs").read_text(encoding="utf-8")
        if not re.search(r"(?m)^\s*#\[cfg\(test\)\]\s*\n\s*mod\s+ccc_tests\s*;", parent):
            raise SystemExit("FALHA: ccc_tests.rs não está incluído exclusivamente sob cfg(test).")
    elif not re.search(r"(?m)^\s*#\[cfg\(test\)\]", source):
        raise SystemExit(f"FALHA: uso de Testcontainers não está isolado em cfg(test): {rel}")
    for name in names:
        leaf = name.rsplit("::", 1)[-1]
        if not re.search(r"#\[\s*tokio::test\s*\]\s*(?:#\[[^\]]+\]\s*)*async\s+fn\s+" + re.escape(leaf) + r"\s*\(", source):
            raise SystemExit(f"FALHA: teste PostgreSQL esperado ausente/não async: {rel}::{leaf}")
        if leaf not in source:
            raise SystemExit(f"FALHA: teste PostgreSQL esperado não encontrado: {rel}::{leaf}")

filters = [name for names in known.values() for name in names]
for runner in ("scripts/mutate-functional-core.sh", "scripts/coverage-functional-core.sh"):
    source = Path(runner).read_text(encoding="utf-8")
    if re.search(r"\bdocker\s+(?:info|run|ps|rm|system)\b|command -v docker", source):
        raise SystemExit(f"FALHA: runner puro invoca/exige Docker: {runner}")
    if 'DOCKER_HOST="unix:///tmp/supplier-rfq-functional-core-must-not-use-docker.sock"' not in source:
        raise SystemExit(f"FALHA: defesa fail-fast DOCKER_HOST ausente: {runner}")
    for name in filters:
        if name not in source:
            raise SystemExit(f"FALHA: filtro PostgreSQL ausente em {runner}: {name}")
    if runner == "scripts/mutate-functional-core.sh":
        dynamic_markers = (
            "scripts/mutation-resource-profile.py",
            "target/cargo-mutants-tmp",
            "CARGO_BUILD_JOBS",
            "CARGO_PROFILE_TEST_DEBUG",
            "MUTANTS_JOBS_SELECTED",
            "--baseline=skip",
        )
        for marker_text in dynamic_markers:
            if marker_text not in source:
                raise SystemExit(f"FALHA: mutation runner sem otimização/recursos dinâmicos: {marker_text}")
if "src/capabilities/finalize_rfq_sourcing/decision.rs" not in Path("scripts/mutate-functional-core.sh").read_text(encoding="utf-8"):
    raise SystemExit("FALHA: decision.rs de FinalizeRfqSourcing ausente do mutation testing.")
if "src/capabilities/finalize_rfq_sourcing/decision.rs" not in Path("scripts/check-functional-core-branch-coverage.py").read_text(encoding="utf-8"):
    raise SystemExit("FALHA: decision.rs de FinalizeRfqSourcing ausente da cobertura.")
resource_helper = Path("scripts/mutation-resource-profile.py")
if not resource_helper.is_file():
    raise SystemExit("FALHA: helper de perfil dinâmico de recursos ausente.")
pure_marker = re.compile(r"Postgres::default|ContainerAsync|testcontainers_modules|\bdocker\s+(?:info|run|ps|rm)\b")
pure = [p for p in Path("src/capabilities").rglob("*.rs") if p.name in {"build_context.rs", "generate_consequences.rs", "decision.rs"}]
violations = [p.as_posix() for p in pure if pure_marker.search(p.read_text(encoding="utf-8"))]
if violations:
    raise SystemExit("FALHA: módulos candidatos do Functional Core contêm dependências de infraestrutura: " + ", ".join(violations))
if "bash scripts/verify-functional-core-test-isolation.sh" not in Path("scripts/verify-rpu-boundaries.sh").read_text(encoding="utf-8"):
    raise SystemExit("FALHA: verify-rpu-boundaries.sh não invoca o guardrail de isolamento.")
print("PASS: inventário completo de nove testes PostgreSQL da library identificado e filtrado.")
print("PASS: mutation/coverage não chamam Docker; DOCKER_HOST inválido protege contra regressão.")
print("PASS: mutation testing detecta CPU/RAM/disco, usa scratch fora de /tmp e valida baseline puro.")
print("PASS: FinalizeRfqSourcing está incluído em mutation testing e branch coverage.")
print("PASS: módulos candidatos do Functional Core não referenciam Testcontainers.")
PY_GUARD
python3 scripts/mutation-resource-profile.py --self-test
