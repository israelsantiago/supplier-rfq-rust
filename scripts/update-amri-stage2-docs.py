#!/usr/bin/env python3
"""Record Stage 2 as complete only after the orchestration script's gates pass."""
from __future__ import annotations

import os
import re
import tempfile
from pathlib import Path

ROOT = Path(os.environ.get("AMRI_REPO_ROOT", Path(__file__).resolve().parents[1])).resolve()
PLAN = ROOT / "docs/architecture/amri/implementation-plan.md"
EVIDENCE = ROOT / "docs/testing/architecture-evidence.md"
ADR = ROOT / "docs/architecture/adr/0034-atomic-multi-rpu-interaction.md"

STAGE2_SECTION = """## Stage 2 — Minimal AMRI Core

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
"""

EVIDENCE_SECTION = """\n\n## AMRI Core — evidências da Etapa 2 (2026-10-30)\n\n| Propriedade | Evidência implementada | Escopo / limite |\n|---|---|---|\n| Identificadores e fingerprints opacos | `identifiers_reject_empty_whitespace_unicode_and_overlong_values`, `fingerprint_hex_round_trips_lower_and_upper_case`, `fingerprint_hex_rejects_wrong_length_and_invalid_characters` | Valida formato e binding; o Core não calcula digest nem define canonicalização. |\n| Reuso de InteractionId | `same_interaction_identity_requires_same_definition_version_and_request_fingerprint` | Detecta reaproveitamento com definição/versão/request diferentes; não persiste marcador. |\n| Grafo de participantes | `interaction_definition_rejects_*`, `interaction_definition_rejects_cycles_and_uses_stable_topological_order`, `topological_sort_handles_multiple_prerequisites_and_stable_ready_queue` | Validação estrutural e ordem topológica estável; sem Reactor/executor. |\n| Outcomes e fatos propostos | `valid_plan_orders_outcomes_and_facts_by_the_declared_dependency_graph`, `planned_fact_reference_exposes_owner_and_typed_payload`, `participant_without_empty_fact_opt_in_cannot_return_no_facts` | Dados tipados e não persistidos; outcome sem fatos exige permissão explícita no contrato do participante. |\n| Plano estrutural | `plan_rejects_definition_identity_and_version_mismatches`, `plan_rejects_unknown_duplicate_and_missing_participants`, `plan_rejects_disallowed_types_duplicate_fact_ids_and_empty_fact_batches` | Não executa Composition Policies de domínio nem valida consistência de storage. |\n| Determinismo | `plan_order_is_independent_of_prepared_outcome_input_permutation` (`proptest`) | Propriedade sobre ordenação; não prova concorrência ou atomicidade. |\n| Isolamento | `verify-amri-core-boundaries.py` + compilação standalone por `rustc` | Produção Core sem crates externas; testes usam apenas `proptest` e não iniciam Testcontainers. |\n| Coverage e mutation | `coverage-amri-core.sh` e `mutate-amri-core.sh` | Alvos limitados a `src/amri`; mutation executa apenas testes da library `amri::`. |\n\nA execução local deve ser registrada no log do ciclo. Uma linha de evidência não autoriza afirmar que snapshot, CCC, commit atômico, idempotência em FACTSTR ou integração AMRI/RFQ já estejam implementados.\n"""


def atomic_write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="") as handle:
            handle.write(content)
        os.replace(name, path)
    except BaseException:
        try:
            os.unlink(name)
        except FileNotFoundError:
            pass
        raise


def replace_stage2(plan: str) -> str:
    start = plan.find("## Stage 2 — Minimal AMRI Core")
    end = plan.find("## Stage 3 — Composite Context and Atomic State Port")
    if start < 0 or end <= start:
        raise SystemExit("FALHA: limites exatos da Etapa 2/3 não encontrados em implementation-plan.md")
    section = plan[start:end]
    if "**Status:** COMPLETED" in section:
        if section.strip() == STAGE2_SECTION.strip():
            return plan
        raise SystemExit("FALHA: Etapa 2 já está marcada de forma divergente; revisão manual necessária")
    if "**Goal:** Implement domain/store-independent protocol primitives only." not in section:
        raise SystemExit("FALHA: conteúdo da Etapa 2 diverge da baseline esperada; nada será sobrescrito")
    return plan[:start] + STAGE2_SECTION + "\n" + plan[end:]


def replace_evidence(evidence: str) -> str:
    stage2_heading = "## AMRI Core — evidências da Etapa 2 (2026-10-30)"
    if stage2_heading in evidence:
        required = (
            "## AMRI — propriedades propostas e estado das evidências",
            "AMRI Core; unit/property tests — implementado na Etapa 2",
            "Sem import/dependência externa em produção; implementado na Etapa 2",
        )
        if all(marker in evidence for marker in required):
            return evidence
        raise SystemExit("FALHA: seção de evidências Stage 2 já existe, mas está divergente")
    old_heading = "## AMRI — propriedades propostas, ainda sem implementação"
    if old_heading not in evidence:
        raise SystemExit("FALHA: seção AMRI da matriz de evidências não encontrada")
    evidence = evidence.replace(
        old_heading,
        "## AMRI — propriedades propostas e estado das evidências",
        1,
    )
    old_intro = (
        "**Importante:** a tabela abaixo é um plano de evidência, não uma afirmação de que os testes ou os módulos já existem. "
        "Os nomes são contratos-alvo a confirmar durante as etapas de implementação da ADR-0034. Até que o código exista e os comandos sejam executados, o estado é `Proposed / Not implemented`."
    )
    new_intro = (
        "**Importante:** esta matriz separa propriedades já demonstradas no Core puro (Etapa 2) de contratos ainda propostos para as etapas seguintes. "
        "Uma propriedade só é promovida quando há teste real, comando executável e limite da prova documentado."
    )
    if old_intro not in evidence:
        raise SystemExit("FALHA: introdução da matriz AMRI diverge da baseline")
    evidence = evidence.replace(old_intro, new_intro, 1)
    old_graph_row = "| Grafo de participantes determinístico | Testes para ciclo, dependência ausente, participante duplicado e ordem estável | AMRI Core; testes puros/propriedade |"
    new_graph_row = "| Grafo de participantes determinístico | `interaction_definition_rejects_*`, `interaction_definition_rejects_cycles_and_uses_stable_topological_order`, `topological_sort_handles_multiple_prerequisites_and_stable_ready_queue`, `plan_order_is_independent_of_prepared_outcome_input_permutation` | AMRI Core; unit/property tests — implementado na Etapa 2 |"
    if old_graph_row not in evidence:
        raise SystemExit("FALHA: linha de grafo de participantes diverge da baseline")
    evidence = evidence.replace(old_graph_row, new_graph_row, 1)
    old_core_row = "| Core sem acoplamento | Build/test do Core sem RFQ, FACTSTR, PostgreSQL, SQLx e Axum | Cargo feature/dependency checks |"
    new_core_row = "| Core sem acoplamento | `verify-amri-core-boundaries.sh` + compilação standalone por `rustc` | Sem import/dependência externa em produção; implementado na Etapa 2. A independência de adapters permanece futura. |"
    if old_core_row not in evidence:
        raise SystemExit("FALHA: linha de isolamento do Core diverge da baseline")
    evidence = evidence.replace(old_core_row, new_core_row, 1)
    return evidence.rstrip() + EVIDENCE_SECTION


def update_adr(adr: str) -> str:
    old = """src/amri/
  mod.rs
  identity.rs
  participant.rs
  execution.rs
  context.rs
  outcome.rs
  composition.rs
  commit_plan.rs
  error.rs
  reactor.rs"""
    new = """src/amri/
  mod.rs
  identity.rs
  definition.rs
  execution.rs
  outcome.rs
  plan.rs
  error.rs
  tests.rs  # cfg(test): unit/property tests only"""
    old_note = (
        "This is a candidate layout only. The first code spike must demonstrate that the "
        "boundaries are useful before creating every module."
    )
    new_note = (
        "Stage 2 implements only the pure protocol primitives listed above. This does not "
        "introduce participant adapters, composite snapshot/token handling, a commit port, "
        "a Reactor or persistence. The Stage 3–5 spikes must validate those boundaries "
        "before additional modules are created."
    )

    if new in adr:
        if new_note in adr:
            return adr
        if adr.count(old_note) == 1:
            return adr.replace(old_note, new_note, 1)
        raise SystemExit("FALHA: layout AMRI já atualizado, mas parágrafo de limite da Etapa 2 diverge")

    if adr.count(old) != 1:
        raise SystemExit("FALHA: layout candidato AMRI na ADR-0034 diverge; nada será sobrescrito")
    if adr.count(old_note) != 1:
        raise SystemExit("FALHA: parágrafo de status do layout AMRI não encontrado exatamente uma vez")
    return adr.replace(old, new, 1).replace(old_note, new_note, 1)


def main() -> int:
    for path in (PLAN, EVIDENCE, ADR):
        if not path.is_file():
            raise SystemExit(f"FALHA: documento obrigatório ausente: {path.relative_to(ROOT)}")
    plan = PLAN.read_text(encoding="utf-8")
    evidence = EVIDENCE.read_text(encoding="utf-8")
    adr = ADR.read_text(encoding="utf-8")

    new_plan = replace_stage2(plan)
    new_evidence = replace_evidence(evidence)
    new_adr = update_adr(adr)

    # All three documents are fully validated before any file is changed.
    originals = {PLAN: PLAN.read_bytes(), EVIDENCE: EVIDENCE.read_bytes(), ADR: ADR.read_bytes()}
    try:
        atomic_write(PLAN, new_plan)
        atomic_write(EVIDENCE, new_evidence)
        atomic_write(ADR, new_adr)
    except BaseException:
        for path, content in originals.items():
            atomic_write(path, content.decode("utf-8"))
        raise
    print("PASS: implementation-plan.md marca somente Stage 2 como concluída.")
    print("PASS: matriz de evidências distingue testes existentes de garantias ainda propostas.")
    print("PASS: ADR-0034 reflete os módulos efetivamente criados e mantém status Proposed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
