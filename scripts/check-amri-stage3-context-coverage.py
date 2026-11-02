#!/usr/bin/env python3
"""Require complete region and branch-outcome coverage for pure Stage 3A modules."""
from __future__ import annotations
import json
import sys
from pathlib import Path

TARGETS = (
    "src/application_state/atomic_interaction/context.rs",
    "src/application_state/atomic_interaction/state_port.rs",
)

def suffix(name: str, target: str) -> bool:
    return name.replace("\\", "/").endswith(target)

def main() -> int:
    if len(sys.argv) != 2:
        print(f"Uso: {sys.argv[0]} target/amri-stage3-context-coverage.json", file=sys.stderr)
        return 2
    try:
        report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"ERRO: JSON llvm-cov inválido: {error}", file=sys.stderr)
        return 2
    data = report.get("data")
    if not isinstance(data, list):
        print("ERRO: JSON llvm-cov sem data[].", file=sys.stderr)
        return 2
    files = [file for item in data for file in item.get("files", [])]
    functions = [fn for item in data for fn in item.get("functions", [])]
    total_regions = covered_regions = total_branches = covered_branches = 0
    failures: list[str] = []

    for target in TARGETS:
        selected = [item for item in files if suffix(str(item.get("filename", "")), target)]
        if not selected:
            failures.append(f"arquivo ausente no relatório: {target}")
            continue
        branch_counts: dict[tuple[int, int, int, int], tuple[int, int]] = {}
        for item in selected:
            for raw in item.get("branches", []):
                if not isinstance(raw, list) or len(raw) < 6:
                    return 2
                key = tuple(int(raw[i]) for i in (0, 1, 2, 3))
                old = branch_counts.get(key, (0, 0))
                branch_counts[key] = (max(old[0], int(raw[4])), max(old[1], int(raw[5])))
        for (line, _column, _end_line, _end_column), (true_count, false_count) in branch_counts.items():
            total_branches += 1
            if true_count > 0 and false_count > 0:
                covered_branches += 1
            else:
                failures.append(f"branch incompleto: {target}:{line} true={true_count} false={false_count}")

        regions: dict[tuple[int, int, int, int], int] = {}
        for fn in functions:
            if not any(suffix(str(name), target) for name in fn.get("filenames", [])):
                continue
            for raw in fn.get("regions", []):
                if not isinstance(raw, list) or len(raw) < 8:
                    return 2
                if int(raw[7]) == 0:
                    key = tuple(int(raw[i]) for i in (0, 1, 2, 3))
                    regions[key] = max(regions.get(key, 0), int(raw[4]))
        if not regions:
            failures.append(f"nenhuma região de produção encontrada: {target}")
        for (line, column, _end_line, _end_column), count in regions.items():
            total_regions += 1
            if count > 0:
                covered_regions += 1
            else:
                failures.append(f"região não executada: {target}:{line}:{column}")

    print(f"AMRI Stage 3A regions: {covered_regions}/{total_regions}")
    print(f"AMRI Stage 3A branch outcomes: {covered_branches}/{total_branches}")
    if not total_regions or not total_branches:
        failures.append("relatório não contém regiões e branches de produção suficientes")
    if failures:
        print("FALHA: cobertura Stage 3A abaixo de 100% ou relatório incompleto:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1
    print("PASS: 100% das regiões e dos dois resultados de cada branch nos módulos puros Stage 3A.")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
