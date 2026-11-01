#!/usr/bin/env python3
"""Require 100% production regions and both outcomes of every AMRI Core branch."""
from __future__ import annotations

import json
import sys
from pathlib import Path

TARGETS = (
    "src/amri/identity.rs",
    "src/amri/definition.rs",
    "src/amri/execution.rs",
    "src/amri/outcome.rs",
    "src/amri/plan.rs",
    "src/amri/error.rs",
)


def suffix_match(filename: str, suffix: str) -> bool:
    return filename.replace("\\", "/").endswith(suffix)


def branch_fields(branch: object) -> tuple[int, int, int, int, int, int]:
    if not isinstance(branch, list) or len(branch) < 9:
        raise ValueError(f"LLVM branch array inválido: {branch!r}")
    return tuple(int(branch[index]) for index in (0, 1, 2, 3, 4, 5))


def region_fields(region: object) -> tuple[int, int, int, int, int, int]:
    if not isinstance(region, list) or len(region) < 8:
        raise ValueError(f"LLVM region array inválido: {region!r}")
    return tuple(int(region[index]) for index in (0, 1, 2, 3, 4, 7))


def main() -> int:
    if len(sys.argv) != 2:
        print(f"Uso: {sys.argv[0]} target/amri-core-branch-coverage.json", file=sys.stderr)
        return 2
    try:
        report = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"ERRO: não foi possível ler JSON llvm-cov: {error}", file=sys.stderr)
        return 2

    data = report.get("data")
    if not isinstance(data, list):
        print("ERRO: JSON llvm-cov sem data[].", file=sys.stderr)
        return 2
    files = [file for item in data for file in item.get("files", [])]
    functions = [function for item in data for function in item.get("functions", [])]

    total_regions = covered_regions = total_branches = covered_branches = 0
    failures: list[str] = []

    for target in TARGETS:
        candidates = [item for item in files if suffix_match(str(item.get("filename", "")), target)]
        if not candidates:
            failures.append(f"arquivo ausente no relatório: {target}")
            continue

        branch_counts: dict[tuple[int, int, int, int], tuple[int, int]] = {}
        for item in candidates:
            for raw in item.get("branches", []):
                try:
                    line, column, end_line, end_column, true_count, false_count = branch_fields(raw)
                except (ValueError, TypeError) as error:
                    print(f"ERRO: {target}: {error}", file=sys.stderr)
                    return 2
                key = (line, column, end_line, end_column)
                old = branch_counts.get(key, (0, 0))
                branch_counts[key] = (max(old[0], true_count), max(old[1], false_count))

        for (line, _column, _end_line, _end_column), (true_count, false_count) in branch_counts.items():
            total_branches += 1
            if true_count > 0 and false_count > 0:
                covered_branches += 1
            else:
                failures.append(
                    f"ramo não exercitado: {target}:{line} true={true_count} false={false_count}"
                )

        code_regions: dict[tuple[int, int, int, int], int] = {}
        for function in functions:
            if not any(suffix_match(str(name), target) for name in function.get("filenames", [])):
                continue
            for raw in function.get("regions", []):
                try:
                    line, column, end_line, end_column, count, kind = region_fields(raw)
                except (ValueError, TypeError) as error:
                    print(f"ERRO: {target}: {error}", file=sys.stderr)
                    return 2
                if kind == 0:
                    key = (line, column, end_line, end_column)
                    code_regions[key] = max(code_regions.get(key, 0), count)

        if not code_regions:
            failures.append(f"nenhuma região de código de produção encontrada: {target}")
        for (line, column, _end_line, _end_column), count in code_regions.items():
            total_regions += 1
            if count > 0:
                covered_regions += 1
            else:
                failures.append(f"região não executada: {target}:{line}:{column}")

    if not total_branches:
        failures.append("nenhum branch de produção apareceu no relatório")
    if not total_regions:
        failures.append("nenhuma região de produção apareceu no relatório")

    print(f"AMRI Core regions: {covered_regions}/{total_regions}")
    print(f"AMRI Core branch outcomes: {covered_branches}/{total_branches}")
    if failures:
        print("FALHA: cobertura AMRI Core abaixo de 100% ou relatório incompleto:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1
    print("PASS: 100% das regiões de produção e dos dois resultados de cada branch AMRI Core.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
