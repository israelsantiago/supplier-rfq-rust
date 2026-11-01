#!/usr/bin/env python3
"""Fail closed if AMRI Core starts depending on application or infrastructure layers."""
from __future__ import annotations

import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PRODUCTION_FILES = (
    "src/amri/identity.rs",
    "src/amri/definition.rs",
    "src/amri/execution.rs",
    "src/amri/outcome.rs",
    "src/amri/plan.rs",
    "src/amri/error.rs",
)
FORBIDDEN = re.compile(
    r"\b(?:factstr|factstr_postgres|sqlx|axum|tokio|chrono|uuid|serde|serde_json|"
    r"testcontainers|PostgresStore|EventStore|EventQuery|PgPool|Rfq|Supplier|PurchaseOrder)\b"
)


def fail(message: str) -> None:
    raise SystemExit(f"FALHA: {message}")


def main() -> int:
    if not (ROOT / "src/amri/mod.rs").is_file():
        fail("src/amri/mod.rs ausente")
    for relative in PRODUCTION_FILES:
        path = ROOT / relative
        if not path.is_file():
            fail(f"módulo AMRI obrigatório ausente: {relative}")
        source = path.read_text(encoding="utf-8")
        if FORBIDDEN.search(source):
            fail(f"dependência/conceito de aplicação ou infraestrutura encontrado em {relative}")
        if "unsafe" in source:
            fail(f"uso/token unsafe encontrado em {relative}")
        if re.search(r"(?m)^\s*use\s+(?!std::|super::|self::|crate::amri::)[A-Za-z_][\w:]*", source):
            fail(f"import externo não permitido em {relative}; produção do Core deve usar somente std e módulos próprios")

    module = (ROOT / "src/amri/mod.rs").read_text(encoding="utf-8")
    if "#[cfg(test)]\nmod tests;" not in module:
        fail("testes do Core devem ficar isolados em src/amri/tests.rs sob cfg(test)")
    tests_path = ROOT / "src/amri/tests.rs"
    if not tests_path.is_file():
        fail("src/amri/tests.rs ausente")
    tests = tests_path.read_text(encoding="utf-8")
    if "proptest::prelude::*" not in tests or "proptest!" not in tests:
        fail("propriedades geradas com proptest ausentes")
    if FORBIDDEN.search(tests):
        fail("teste AMRI contém conceito de domínio ou infraestrutura proibido")
    if re.search(r"\b(?:ContainerAsync|Postgres::default|AsyncRunner|docker\s+(?:run|ps|info))\b", tests):
        fail("testes AMRI tentam iniciar infraestrutura/Testcontainers")

    lib_rs = (ROOT / "src/lib.rs").read_text(encoding="utf-8")
    if not re.search(r"(?m)^pub\s+mod\s+amri;\s*$", lib_rs):
        fail("src/lib.rs não expõe pub mod amri;")

    cargo_toml = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    if not re.search(r"(?ms)^\[dev-dependencies\].*?^proptest\s*=\s*\"=1\.11\.0\"\s*$", cargo_toml):
        fail('proptest = "=1.11.0" não está declarado nas dev-dependencies existentes')

    rustc = shutil.which("rustc")
    if not rustc:
        fail("rustc indisponível; não é possível provar compilação standalone do módulo")
    target = ROOT / "target/amri-core-standalone"
    target.mkdir(parents=True, exist_ok=True)
    command = [
        rustc,
        "--edition=2024",
        "--crate-type=lib",
        "--crate-name=amri_core",
        "--deny=warnings",
        str(ROOT / "src/amri/mod.rs"),
        "-o",
        str(target / "libamri_core_standalone.rlib"),
    ]
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=False)
    if result.returncode != 0:
        sys.stderr.write(result.stdout)
        sys.stderr.write(result.stderr)
        fail("AMRI Core não compila isoladamente com rustc e sem crates externas")

    print("PASS: AMRI Core não importa RPU, Supply Chain, FACTSTR, PostgreSQL, SQLx, Axum ou Tokio.")
    print("PASS: os testes do Core são unitários/property-based e não contêm Testcontainers.")
    print("PASS: rustc compilou src/amri/mod.rs isoladamente sem dependências externas.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
