#!/usr/bin/env python3
"""Choose safe cargo-mutants parallelism from live CPU, memory, and filesystem capacity.

The estimates intentionally reserve resources for the interactive desktop and account
for one separate Cargo target directory per cargo-mutants worker.
"""
from __future__ import annotations

import argparse
import json
import math
import os
import shutil
import sys
from pathlib import Path
from typing import Any

GIB = 1024 ** 3
MIN_MEMORY_GIB = 4
MIN_DISK_GIB = 6
MEMORY_PER_MUTANT_GIB = 4
DISK_PER_MUTANT_GIB = 4
MEMORY_PER_CARGO_BUILD_GIB = 3


def _read_int(path: Path) -> int | None:
    try:
        value = path.read_text(encoding="utf-8").strip()
        if value.isdigit():
            return int(value)
    except OSError:
        pass
    return None


def _physical_cores(allowed_cpus: set[int]) -> int | None:
    """Count physical core/package pairs visible through the current CPU affinity."""
    pairs: set[tuple[str, str]] = set()
    for cpu in allowed_cpus:
        topology = Path(f"/sys/devices/system/cpu/cpu{cpu}/topology")
        package = _read_int(topology / "physical_package_id")
        core = _read_int(topology / "core_id")
        if package is not None and core is not None:
            pairs.add((str(package), str(core)))
    if pairs:
        return len(pairs)

    # Linux /proc/cpuinfo fallback; count only processor IDs allowed by affinity.
    try:
        text = Path("/proc/cpuinfo").read_text(encoding="utf-8")
        for block in text.split("\n\n"):
            fields: dict[str, str] = {}
            for line in block.splitlines():
                if ":" in line:
                    key, value = line.split(":", 1)
                    fields[key.strip()] = value.strip()
            processor = fields.get("processor")
            package = fields.get("physical id")
            core = fields.get("core id")
            if processor and package is not None and core is not None and int(processor) in allowed_cpus:
                pairs.add((package, core))
    except (OSError, ValueError):
        pass
    return len(pairs) if pairs else None


def _effective_cpu_count() -> tuple[int, int, int, str]:
    """Return effective physical worker cores, affinity CPUs, physical core count, note."""
    affinity: set[int] | None = None
    if hasattr(os, "sched_getaffinity"):
        try:
            affinity = set(os.sched_getaffinity(0))
        except OSError:
            pass
    if not affinity:
        count = os.cpu_count() or 1
        affinity = set(range(count))

    affinity_count = max(1, len(affinity))
    physical = _physical_cores(affinity) or affinity_count
    effective = max(1, min(affinity_count, physical))
    notes = [f"affinity/logical={affinity_count}", f"physical_cores={physical}"]

    # Cgroup v2 CPU quota, when present, is a stricter bound than topology/affinity.
    try:
        fields = Path("/sys/fs/cgroup/cpu.max").read_text(encoding="utf-8").split()
        if len(fields) == 2 and fields[0] != "max":
            quota, period = int(fields[0]), int(fields[1])
            if quota > 0 and period > 0:
                quota_cpus = max(1, quota // period)
                effective = min(effective, quota_cpus)
                notes.append(f"cgroup_quota={quota_cpus}")
    except (OSError, ValueError):
        pass

    # Cgroup v1 fallback. Ignore the very large sentinel used for unlimited quotas.
    quota_path = Path("/sys/fs/cgroup/cpu/cpu.cfs_quota_us")
    period_path = Path("/sys/fs/cgroup/cpu/cpu.cfs_period_us")
    quota = _read_int(quota_path)
    period = _read_int(period_path)
    if quota is not None and period and quota > 0 and quota < (1 << 60):
        quota_cpus = max(1, quota // period)
        effective = min(effective, quota_cpus)
        notes.append(f"cgroup_v1_quota={quota_cpus}")

    return max(1, effective), affinity_count, physical, ", ".join(notes)


def _available_memory_bytes() -> tuple[int, str]:
    """Use host MemAvailable, limited by remaining cgroup memory if applicable."""
    mem_available: int | None = None
    try:
        for line in Path("/proc/meminfo").read_text(encoding="utf-8").splitlines():
            if line.startswith("MemAvailable:"):
                mem_available = int(line.split()[1]) * 1024
                break
    except (OSError, ValueError, IndexError):
        pass

    notes: list[str] = []
    if mem_available is not None:
        notes.append("/proc/meminfo:MemAvailable")

    # Cgroup v2 maximum is bytes; constrain host MemAvailable by remaining cgroup bytes.
    limit = _read_int(Path("/sys/fs/cgroup/memory.max"))
    current = _read_int(Path("/sys/fs/cgroup/memory.current"))
    if limit is not None and current is not None and limit > 0:
        remaining = max(0, limit - current)
        mem_available = remaining if mem_available is None else min(mem_available, remaining)
        notes.append("cgroup_v2_memory")

    # Cgroup v1 fallback.
    v1_limit = _read_int(Path("/sys/fs/cgroup/memory/memory.limit_in_bytes"))
    v1_current = _read_int(Path("/sys/fs/cgroup/memory/memory.usage_in_bytes"))
    if (
        v1_limit is not None
        and v1_current is not None
        and v1_limit > 0
        and v1_limit < (1 << 60)
    ):
        remaining = max(0, v1_limit - v1_current)
        mem_available = remaining if mem_available is None else min(mem_available, remaining)
        notes.append("cgroup_v1_memory")

    if mem_available is None:
        raise RuntimeError("não foi possível detectar memória disponível em /proc/meminfo ou cgroups")
    return mem_available, ", ".join(notes) or "detecção de memória"


def calculate_profile(
    cpu_count: int,
    available_memory_bytes: int,
    available_disk_bytes: int,
    requested_jobs: str | int | None = None,
    max_jobs: str | int | None = None,
) -> dict[str, Any]:
    """Pure resource calculation, separated for deterministic synthetic testing."""
    if cpu_count < 1:
        raise ValueError("CPUs disponíveis deve ser maior que zero")
    if available_memory_bytes < MIN_MEMORY_GIB * GIB:
        raise ValueError(
            f"memória disponível abaixo de {MIN_MEMORY_GIB} GiB; feche processos pesados ou libere memória"
        )
    if available_disk_bytes < MIN_DISK_GIB * GIB:
        raise ValueError(
            f"espaço livre abaixo de {MIN_DISK_GIB} GiB no filesystem temporário escolhido; use MUTANTS_TMPDIR em outro disco com mais espaço"
        )

    memory_gib = available_memory_bytes // GIB
    disk_gib = available_disk_bytes // GIB
    memory_worker_limit = max(1, memory_gib // MEMORY_PER_MUTANT_GIB)
    disk_worker_limit = max(1, disk_gib // DISK_PER_MUTANT_GIB)
    safe_jobs = max(1, min(cpu_count, memory_worker_limit, disk_worker_limit))

    if max_jobs not in (None, "", "auto"):
        try:
            configured_max = int(max_jobs)
        except (TypeError, ValueError) as exc:
            raise ValueError("MUTANTS_MAX_JOBS deve ser um inteiro positivo ou 'auto'") from exc
        if configured_max < 1:
            raise ValueError("MUTANTS_MAX_JOBS deve ser maior que zero")
        safe_jobs = min(safe_jobs, configured_max)

    clamped = False
    requested: int | None = None
    if requested_jobs not in (None, "", "auto"):
        try:
            requested = int(requested_jobs)
        except (TypeError, ValueError) as exc:
            raise ValueError("MUTANTS_JOBS deve ser inteiro positivo ou 'auto'") from exc
        if requested < 1:
            raise ValueError("MUTANTS_JOBS deve ser maior que zero")
        jobs = min(requested, safe_jobs)
        clamped = jobs != requested
    else:
        jobs = safe_jobs

    # cargo-mutants workers are outer parallelism; Cargo builds are inner parallelism.
    # Bound total compiler slots by both CPU affinity/quota and memory budget.
    compile_slots = max(1, min(cpu_count, memory_gib // MEMORY_PER_CARGO_BUILD_GIB))
    cargo_build_jobs = max(1, min(2, compile_slots // jobs, cpu_count // jobs or 1))
    return {
        "cpu_count": cpu_count,
        "memory_gib": int(memory_gib),
        "disk_gib": int(disk_gib),
        "memory_worker_limit": int(memory_worker_limit),
        "disk_worker_limit": int(disk_worker_limit),
        "safe_job_limit": int(safe_jobs),
        "requested_jobs": requested if requested is not None else "auto",
        "mutants_jobs": int(jobs),
        "cargo_build_jobs": int(cargo_build_jobs),
        "clamped": clamped,
        "debug_info": 0,
    }


def live_profile(scratch: Path) -> dict[str, Any]:
    scratch.mkdir(parents=True, exist_ok=True)
    cpu_count, affinity_count, physical_cores, cpu_note = _effective_cpu_count()
    memory_bytes, memory_note = _available_memory_bytes()
    disk_bytes = shutil.disk_usage(scratch).free
    profile = calculate_profile(
        cpu_count=cpu_count,
        available_memory_bytes=memory_bytes,
        available_disk_bytes=disk_bytes,
        requested_jobs=os.environ.get("MUTANTS_JOBS"),
        max_jobs=os.environ.get("MUTANTS_MAX_JOBS"),
    )
    profile.update({
        "affinity_cpus": affinity_count,
        "physical_cores": physical_cores,
        "cpu_detection": cpu_note,
        "memory_detection": memory_note,
        "available_memory_bytes": memory_bytes,
        "available_disk_bytes": disk_bytes,
        "scratch": str(scratch.resolve()),
    })
    return profile


def self_test() -> int:
    # Also validate live CPU topology/affinity detection on the host executing this test.
    detected, affinity, physical, _note = _effective_cpu_count()
    assert detected >= 1 and affinity >= detected and physical >= 1 and physical >= detected, (
        detected, affinity, physical
    )

    # Synthetic 8-core machine with enough RAM/disk: use all 8 workers dynamically.
    p = calculate_profile(8, 40 * GIB, 48 * GIB)
    assert p["cpu_count"] == 8 and p["mutants_jobs"] == 8 and p["cargo_build_jobs"] == 1, p

    # Memory or disk pressure clamps workers instead of oversubscribing.
    p = calculate_profile(8, 16 * GIB, 40 * GIB)
    assert p["mutants_jobs"] == 4 and p["cargo_build_jobs"] == 1, p
    p = calculate_profile(8, 32 * GIB, 12 * GIB)
    assert p["mutants_jobs"] == 3, p

    # Explicit overrides are respected only within the safe resource ceiling.
    p = calculate_profile(8, 16 * GIB, 40 * GIB, requested_jobs="8")
    assert p["requested_jobs"] == 8 and p["mutants_jobs"] == 4 and p["clamped"], p
    p = calculate_profile(8, 32 * GIB, 48 * GIB, requested_jobs="2")
    assert p["mutants_jobs"] == 2 and not p["clamped"], p

    for args in ((0, 16 * GIB, 16 * GIB), (8, 2 * GIB, 16 * GIB), (8, 16 * GIB, 4 * GIB)):
        try:
            calculate_profile(*args)
        except ValueError:
            pass
        else:
            raise AssertionError(f"perfil deveria rejeitar recursos insuficientes: {args}")
    print(f"PASS: CPUs detectados dinamicamente neste host: efetivos={detected}, afinidade/logical={affinity}, físicos={physical}.")
    print("PASS: perfil sintético de 8 CPUs cobre limites de RAM/disco, override e falta de recursos.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scratch", type=Path, help="filesystem onde o cargo-mutants criará seus trees temporários")
    parser.add_argument("--shell", action="store_true", help="emite apenas variáveis numéricas para o runner Bash")
    parser.add_argument("--self-test", action="store_true", help="executa testes sintéticos de cálculo")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    if args.scratch is None:
        parser.error("--scratch é obrigatório")
    try:
        p = live_profile(args.scratch)
    except (OSError, RuntimeError, ValueError) as exc:
        print(f"ERRO: perfil de recursos não calculado: {exc}", file=sys.stderr)
        return 2
    if args.shell:
        for key, env_name in (
            ("cpu_count", "CPU_COUNT"),
            ("affinity_cpus", "AFFINITY_CPU_COUNT"),
            ("physical_cores", "PHYSICAL_CORE_COUNT"),
            ("memory_gib", "MEMORY_GIB"),
            ("disk_gib", "DISK_GIB"),
            ("mutants_jobs", "MUTANTS_JOBS_SELECTED"),
            ("cargo_build_jobs", "CARGO_BUILD_JOBS_SELECTED"),
            ("safe_job_limit", "MUTANTS_SAFE_JOB_LIMIT"),
            ("clamped", "MUTANTS_JOBS_CLAMPED"),
        ):
            print(f"{env_name}={p[key]}")
        return 0
    print(json.dumps(p, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, OSError) as exc:
        print(f"ERRO: {exc}", file=sys.stderr)
        raise SystemExit(2)
