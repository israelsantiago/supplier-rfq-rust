#!/usr/bin/env python3
"""Fail unless production Functional Core code regions and branch outcomes are covered.

LLVM llvm-cov export JSON encodes branches and regions as positional arrays.
See LLVM CoverageExporterJson.cpp. The checker intentionally fails closed when
an unexpected schema is encountered instead of reporting false 100% coverage.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

TARGETS = (
    "src/capabilities/open_rfq/build_context.rs",
    "src/capabilities/open_rfq/generate_consequences.rs",
    "src/capabilities/register_supplier/build_context.rs",
    "src/capabilities/register_supplier/generate_consequences.rs",
    "src/capabilities/set_supplier_eligibility/build_context.rs",
    "src/capabilities/set_supplier_eligibility/generate_consequences.rs",
    "src/capabilities/close_rfq/build_context.rs",
    "src/capabilities/close_rfq/generate_consequences.rs",
    "src/capabilities/submit_supplier_quote/build_context.rs",
    "src/capabilities/submit_supplier_quote/decision.rs",
    "src/capabilities/list_rfq_summaries/build_context.rs",
    "src/capabilities/list_supplier_quotes/build_context.rs",
)


def suffix_match(filename: str, suffix: str) -> bool:
    return filename.replace("\\", "/").endswith(suffix)


def branch_fields(branch: object) -> tuple[int, int, int, int, int, int]:
    """Return (line_start, col_start, line_end, col_end, true_count, false_count)."""
    if isinstance(branch, list):
        # LLVM CountedRegion branch tuple:
        # [line_start, col_start, line_end, col_end, execution_count,
        #  false_execution_count, file_id, expanded_file_id, kind]
        if len(branch) < 9:
            raise ValueError(f"LLVM branch array too short: {branch!r}")
        return tuple(int(branch[i]) for i in (0, 1, 2, 3, 4, 5))
    if isinstance(branch, dict):
        # Defensive support for future/alternate exporters that use named fields.
        line = branch.get("line_number", branch.get("lineNumber", branch.get("line")))
        col = branch.get("column_number", branch.get("columnNumber", branch.get("column", 0)))
        line_end = branch.get("end_line", branch.get("endLine", line))
        col_end = branch.get("end_column", branch.get("endColumn", col))
        true_count = branch.get("true_count", branch.get("trueCount", branch.get("execution_count")))
        false_count = branch.get("false_count", branch.get("falseCount", branch.get("false_execution_count")))
        if line is None or true_count is None or false_count is None:
            raise ValueError(f"Unrecognized branch object: {branch!r}")
        return int(line), int(col), int(line_end), int(col_end), int(true_count), int(false_count)
    raise ValueError(f"Unrecognized LLVM branch format: {branch!r}")


def region_fields(region: object) -> tuple[int, int, int, int, int, int]:
    """Return (start_line, start_col, end_line, end_col, count, kind)."""
    if not isinstance(region, list) or len(region) < 8:
        raise ValueError(f"Unrecognized LLVM region format: {region!r}")
    # LLVM CountedRegion tuple:
    # [line_start, col_start, line_end, col_end, execution_count,
    #  file_id, expanded_file_id, kind]
    return int(region[0]), int(region[1]), int(region[2]), int(region[3]), int(region[4]), int(region[7])


def main() -> int:
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} target/functional-core-branch-coverage.json", file=sys.stderr)
        return 2
    try:
        report = json.loads(Path(sys.argv[1]).read_text())
    except (OSError, json.JSONDecodeError) as exc:
        print(f"ERRO: não foi possível ler o JSON de coverage: {exc}", file=sys.stderr)
        return 2

    data = report.get("data")
    if not isinstance(data, list):
        print("ERRO: o relatório LLVM não contém data[].", file=sys.stderr)
        return 2

    files = [file for item in data for file in item.get("files", [])]
    functions = [function for item in data for function in item.get("functions", [])]
    missing_files: list[str] = []
    uncovered_branches: list[tuple[str, int, int, int]] = []
    uncovered_regions: list[tuple[str, int, int, int]] = []
    total_branches = 0
    covered_branches = 0
    total_regions = 0
    covered_regions = 0

    for suffix in TARGETS:
        file_candidates = [f for f in files if suffix_match(str(f.get("filename", "")), suffix)]
        if not file_candidates:
            missing_files.append(suffix)
            continue

        try:
            source_lines = Path(suffix).read_text().splitlines()
        except OSError as exc:
            print(f"ERRO: arquivo do escopo não está no checkout: {suffix}: {exc}", file=sys.stderr)
            return 2
        test_start = next(
            (idx + 1 for idx, line in enumerate(source_lines) if line.strip() == "#[cfg(test)]"),
            len(source_lines) + 1,
        )

        # Branches are emitted per source file, and may repeat if the compiler exports
        # multiple mappings. Merge the counters by source location, taking maximums.
        branch_counts: dict[tuple[int, int, int, int], tuple[int, int]] = {}
        for file in file_candidates:
            for raw_branch in file.get("branches", []):
                try:
                    line, column, line_end, column_end, true_count, false_count = branch_fields(raw_branch)
                except (ValueError, TypeError) as exc:
                    print(f"ERRO: formato de branch não reconhecido em {suffix}: {exc}", file=sys.stderr)
                    return 2
                if line >= test_start:
                    continue
                location = (line, column, line_end, column_end)
                prior = branch_counts.get(location)
                if prior is None:
                    branch_counts[location] = (true_count, false_count)
                else:
                    branch_counts[location] = (max(prior[0], true_count), max(prior[1], false_count))

        for location, (true_count, false_count) in branch_counts.items():
            line = location[0]
            total_branches += 1
            if true_count > 0 and false_count > 0:
                covered_branches += 1
            else:
                uncovered_branches.append((suffix, line, true_count, false_count))

        # Region arrays are attached to function entries rather than file entries.
        # Only kind=0 is CodeRegion; skipped/gap/expansion regions are not code branches.
        file_regions: dict[tuple[int, int, int, int], int] = {}
        for function in functions:
            filenames = function.get("filenames", [])
            if not any(suffix_match(str(filename), suffix) for filename in filenames):
                continue
            for raw_region in function.get("regions", []):
                try:
                    line_start, col_start, line_end, col_end, count, kind = region_fields(raw_region)
                except (ValueError, TypeError) as exc:
                    print(f"ERRO: formato de região não reconhecido em {suffix}: {exc}", file=sys.stderr)
                    return 2
                if kind != 0 or line_start >= test_start:
                    continue
                key = (line_start, col_start, line_end, col_end)
                file_regions[key] = max(file_regions.get(key, 0), count)

        if not file_regions:
            print(f"ERRO: não encontrei regiões de código de produção para {suffix}; formato/escopo pode ter mudado.", file=sys.stderr)
            return 2
        for (line_start, col_start, _line_end, _col_end), count in file_regions.items():
            total_regions += 1
            if count > 0:
                covered_regions += 1
            else:
                uncovered_regions.append((suffix, line_start, col_start, count))

    if missing_files:
        print("ERRO: estes arquivos do Functional Core não apareceram no relatório:", file=sys.stderr)
        for path in missing_files:
            print(f"  - {path}", file=sys.stderr)
        return 2
    if total_branches == 0:
        print("ERRO: nenhum branch de produção foi encontrado; não é possível declarar 100%.", file=sys.stderr)
        return 2
    if total_regions == 0:
        print("ERRO: nenhuma região de código de produção foi encontrada; não é possível declarar 100%.", file=sys.stderr)
        return 2

    branch_percent = 100.0 * covered_branches / total_branches
    region_percent = 100.0 * covered_regions / total_regions
    print(f"Functional Core region coverage: {covered_regions}/{total_regions} regiões de código cobertas ({region_percent:.2f}%).")
    print(f"Functional Core branch coverage: {covered_branches}/{total_branches} ramos com ambos os resultados exercitados ({branch_percent:.2f}%).")

    if uncovered_regions:
        print("Regiões de código não cobertas:")
        for path, line, col, count in uncovered_regions:
            print(f"  {path}:{line}:{col} — execution_count={count}")
    if uncovered_branches:
        print("Ramos/condições sem cobertura completa:")
        for path, line, true_count, false_count in uncovered_branches:
            print(f"  {path}:{line} — true_count={true_count}, false_count={false_count}")

    return 0 if not uncovered_regions and not uncovered_branches else 1


if __name__ == "__main__":
    raise SystemExit(main())
