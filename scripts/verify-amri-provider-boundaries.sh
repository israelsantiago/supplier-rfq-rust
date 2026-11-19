#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

python3 - <<'PY'
from pathlib import Path
import re

required = (
    Path("src/app/amri/providers/mod.rs"),
    Path("src/app/amri/providers/testing.rs"),
    Path("tests/external_provider_contracts.rs"),
    Path("tests/wiremock-rs/src/main.rs"),
    Path("tests/contracts/external-providers-v1.openapi.yaml"),
)
for path in required:
    if not path.is_file() or path.stat().st_size == 0:
        raise SystemExit(f"FAIL: arquivo ausente/vazio: {path}")

core_files = [*Path("src/amri").rglob("*.rs"), *Path("src/domain").rglob("*.rs")]
participant_files = list(Path("src/amri_participants").rglob("*.rs"))
forbidden_core = re.compile(r"wiremock|testcontainers|reqwest|crate::app::amri::providers|crate::app::amri::provider_test_support", re.I)
for path in core_files:
    source = path.read_text(encoding="utf-8")
    if forbidden_core.search(source):
        raise SystemExit(f"FAIL: dependência externa/Application encontrada no Core/Functional Core: {path}")
for path in participant_files:
    source = path.read_text(encoding="utf-8")
    if forbidden_core.search(source):
        raise SystemExit(f"FAIL: Prepared Participant depende de Provider/HTTP: {path}")

root_manifest = __import__("tomllib").loads(Path("Cargo.toml").read_text(encoding="utf-8"))
if "testcontainers" in root_manifest.get("dependencies", {}):
    raise SystemExit("FAIL: Testcontainers não pode ser dependência de produção")
if root_manifest.get("dev-dependencies", {}).get("testcontainers") != "=0.27.3":
    raise SystemExit("FAIL: Testcontainers Rust deve ser dev-dependency direta e pinada em 0.27.3")
providers = Path("src/app/amri/providers/mod.rs").read_text(encoding="utf-8")
for token in ("pub trait ErpReferenceProvider", "pub trait ComplianceProvider", "pub trait TaxProvider", "NotFound", "NotApplicable", "Unavailable", "TimedOut", "InvalidResponse", "EvidenceFreshness", "Stale", "freshness_at", "observed_at", "expires_at"):
    if token not in providers:
        raise SystemExit(f"FAIL: contrato de Provider incompleto: {token}")
for forbidden in ("factstr::", "sqlx::", "EventStore", "append_if", "reqwest::", "wiremock::", "testcontainers"):
    if forbidden in providers:
        raise SystemExit(f"FAIL: porta Provider contém dependência/efeito proibido: {forbidden}")

amri = Path("src/app/amri/mod.rs").read_text(encoding="utf-8")
if "pub(crate) mod providers;" not in amri:
    raise SystemExit("FAIL: módulo Providers deve permanecer pub(crate)")
if '#[cfg(feature = "test-support")]' not in amri or "provider_test_support" not in amri:
    raise SystemExit("FAIL: reexport de teste deve permanecer feature-gated")

def mask_rust_non_code(source: str) -> str:
    """Mask Rust comments and string literals while preserving offsets/newlines."""
    chars = list(source)
    size = len(source)
    index = 0

    def blank(start: int, end: int) -> None:
        for pos in range(start, min(end, size)):
            if chars[pos] not in "\r\n":
                chars[pos] = " "

    def identifier_char(value: str) -> bool:
        return value.isalnum() or value == "_"

    while index < size:
        if source.startswith("//", index):
            end = source.find("\n", index)
            if end < 0:
                end = size
            blank(index, end)
            index = end
            continue

        if source.startswith("/*", index):
            depth = 1
            cursor = index + 2
            while cursor < size and depth:
                if source.startswith("/*", cursor):
                    depth += 1
                    cursor += 2
                elif source.startswith("*/", cursor):
                    depth -= 1
                    cursor += 2
                else:
                    cursor += 1
            if depth:
                raise ValueError("comentário Rust de bloco não terminado")
            blank(index, cursor)
            index = cursor
            continue

        # Mask character literals too; without this, a character containing a
        # double quote (for example '"') could be mistaken for a string opener.
        if source[index] == "'":
            cursor = index + 1
            if cursor < size and source[cursor] == "\\":
                cursor += 1
                if cursor < size:
                    if source[cursor] == "x":
                        cursor += 3
                    elif source[cursor] == "u" and cursor + 1 < size and source[cursor + 1] == "{":
                        close = source.find("}", cursor + 2)
                        cursor = size if close < 0 else close + 1
                    else:
                        cursor += 1
            elif cursor < size:
                cursor += 1
            if cursor < size and source[cursor] == "'":
                end = cursor + 1
                blank(index, end)
                index = end
                continue

        boundary = index == 0 or not identifier_char(source[index - 1])
        raw_prefix = None
        if boundary:
            for prefix in ("br", "cr", "r"):
                if source.startswith(prefix, index):
                    cursor = index + len(prefix)
                    while cursor < size and source[cursor] == "#":
                        cursor += 1
                    if cursor < size and source[cursor] == '"':
                        raw_prefix = (prefix, cursor - (index + len(prefix)))
                        break
        if raw_prefix is not None:
            prefix, hashes = raw_prefix
            opening_quote = index + len(prefix) + hashes
            terminator = '"' + ("#" * hashes)
            end_quote = source.find(terminator, opening_quote + 1)
            if end_quote < 0:
                raise ValueError("string raw Rust não terminada")
            end = end_quote + len(terminator)
            blank(index, end)
            index = end
            continue

        string_prefix = None
        if source[index] == '"':
            string_prefix = ""
        elif boundary and source.startswith('b"', index):
            string_prefix = "b"
        elif boundary and source.startswith('c"', index):
            string_prefix = "c"
        if string_prefix is not None:
            quote = index + len(string_prefix)
            cursor = quote + 1
            while cursor < size:
                if source[cursor] == "\\":
                    cursor += 2
                elif source[cursor] == '"':
                    cursor += 1
                    break
                else:
                    cursor += 1
            else:
                raise ValueError("string Rust não terminada")
            blank(index, cursor)
            index = cursor
            continue

        index += 1

    return "".join(chars)


def _module_child_candidates(parent: Path, module_name: str) -> tuple[Path, Path]:
    if parent.name in {"mod.rs", "lib.rs", "main.rs"}:
        base = parent.parent
    else:
        base = parent.parent / parent.stem
    return (base / f"{module_name}.rs", base / module_name / "mod.rs")


def mask_cfg_test_inline_modules(source: str) -> tuple[str, list[str]]:
    """Mask top-level inline cfg(test) modules and report external cfg(test) modules."""
    code = mask_rust_non_code(source)
    chars = list(code)
    pattern = re.compile(
        r"(?m)^[ \t]*#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]"
        r"\s*(?:#\s*\[[^\]]*\]\s*)*"
        r"(?:(?:pub(?:\s*\([^)]*\))?)\s+)?"
        r"mod\s+(?P<name>[A-Za-z_]\w*)\s*(?P<kind>[{;])"
    )
    masked_ranges: list[tuple[int, int]] = []
    external_modules: list[str] = []

    for match in pattern.finditer(code):
        if any(start <= match.start() < end for start, end in masked_ranges):
            continue
        depth = code[:match.start()].count("{") - code[:match.start()].count("}")
        if depth != 0:
            # Nested-module filesystem resolution is deliberately fail-closed: the
            # child remains in the production scan instead of being exempted wrongly.
            continue
        kind = match.group("kind")
        if kind == ";":
            external_modules.append(match.group("name"))
            continue

        opening = match.end() - 1
        brace_depth = 0
        closing = None
        for cursor in range(opening, len(code)):
            if code[cursor] == "{":
                brace_depth += 1
            elif code[cursor] == "}":
                brace_depth -= 1
                if brace_depth == 0:
                    closing = cursor + 1
                    break
        if closing is None:
            raise ValueError(
                f"módulo cfg(test) sem fechamento de chaves: {match.group('name')}"
            )
        masked_ranges.append((match.start(), closing))
        for pos in range(match.start(), closing):
            if chars[pos] not in "\r\n":
                chars[pos] = " "

    return "".join(chars), external_modules


def production_scan_sources(root: Path) -> tuple[list[Path], set[Path]]:
    """Return Rust source files and files reachable only from cfg(test) modules."""
    sources = sorted(root.rglob("*.rs"))
    test_only: set[Path] = set()
    parents_to_scan = list(sources)
    visited: set[Path] = set()

    while parents_to_scan:
        parent = parents_to_scan.pop()
        if parent in visited:
            continue
        visited.add(parent)
        try:
            _, external_modules = mask_cfg_test_inline_modules(
                parent.read_text(encoding="utf-8")
            )
        except (OSError, UnicodeError, ValueError) as exc:
            raise RuntimeError(f"não foi possível analisar módulos Rust em {parent}: {exc}") from exc
        for module_name in external_modules:
            for candidate in _module_child_candidates(parent, module_name):
                if candidate.is_file() and candidate not in test_only:
                    test_only.add(candidate)
                    # A child file is itself compiled only under the parent's cfg(test).
                    # Its declared child modules inherit that test-only compilation.
                    parents_to_scan.append(candidate)

    return sources, test_only

sources, test_only_modules = production_scan_sources(Path("src"))
for path in sources:
    if path.parts[:2] == ("src", "amri") or path.parts[:2] == ("src", "domain"):
        continue
    if path.parts[:3] == ("src", "app", "amri") and "providers" in path.parts:
        continue
    if path in test_only_modules:
        continue
    raw_source = path.read_text(encoding="utf-8")
    try:
        production_source, _ = mask_cfg_test_inline_modules(raw_source)
    except (OSError, UnicodeError, ValueError) as exc:
        raise SystemExit(f"FAIL: não foi possível separar produção/testes em {path}: {exc}")
    if re.search(r"wiremock::|testcontainers(?:_modules)?::|GenericImage::new", production_source):
        raise SystemExit(f"FAIL: infraestrutura WireMock/Testcontainers vazou para produção: {path}")

openapi = Path("tests/contracts/external-providers-v1.openapi.yaml").read_text(encoding="utf-8")
for path in ("/v1/erp-reference/suppliers/{supplier_id}/reference", "/v1/compliance/suppliers/{supplier_id}/evidence", "/v1/tax/suppliers/{supplier_id}/evidence"):
    if path not in openapi:
        raise SystemExit(f"FAIL: rota de contrato ausente: {path}")
for invented in ("tax_rate", "tax_code", "risk_score", "compliance_status", "purchase_order_create"):
    if invented in openapi:
        raise SystemExit(f"FAIL: campo/regra não aprovado entrou no contrato: {invented}")

print("PASS: Providers residem somente na Application; Core/Functional Core/participants permanecem sem HTTP e infraestrutura.")
print("PASS: contratos síncronos, read-only, tipados e Fakes feature-gated.")
print("PASS: OpenAPI contém somente referências externas e proveniência; nenhuma regra fiscal/compliance foi inventada.")
PY
