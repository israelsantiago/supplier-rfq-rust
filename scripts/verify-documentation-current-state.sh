#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
python3 - <<'PY_CHECK'
from pathlib import Path
import re

readme = Path('README.md').read_text()
design = Path('docs/design-approach.md').read_text()
ci = Path('.github/workflows/ci.yml').read_text()
adr_index = Path('docs/architecture/adr/README.md').read_text()

stale_readme = [
    'Esta branch adiciona Event Sourcing',
    'preservada na branch `main`',
    '`main`: implementação relacional original',
    '`event-sourcing-factstr`: esta variante',
    'Não há idempotency key para repetições de rede',
]
for phrase in stale_readme:
    if phrase in readme:
        raise SystemExit(f'ERRO: afirmação obsoleta no README: {phrase}')
if 'branch `event-sourcing-factstr`' in design:
    raise SystemExit('ERRO: o guia de design aponta para a branch obsoleta.')
if 'event-sourcing-factstr' in ci:
    raise SystemExit('ERRO: o workflow ainda filtra a branch removida.')
branches = re.findall(r'branches:\s*\[([^\]]+)\]', ci)
if len(branches) < 2 or any([b.strip() != 'main' for b in branches]):
    raise SystemExit(f'ERRO: filtros de push/PR precisam apontar somente para main; encontrados: {branches}')
for number, filename in [
    ('0028', '0028-http-error-redaction-request-correlation.md'),
    ('0029', '0029-mainline-documentation-and-ci.md'),
    ('0030', '0030-rfq-summary-reader-port.md'),
    ('0031', '0031-atomic-rfq-sourcing-finalization.md'),
]:
    path = Path('docs/architecture/adr') / filename
    if not path.is_file():
        raise SystemExit(f'ERRO: ADR-{number} não encontrado: {path}')
    if f'ADR-{number}' not in adr_index:
        raise SystemExit(f'ERRO: ADR-{number} não está indexado em docs/architecture/adr/README.md')
flow = Path('docs/flows/atomic-rfq-sourcing.md')
if not flow.is_file() or 'append_if' not in flow.read_text() or 'sequenceDiagram' not in flow.read_text() or 'flowchart' not in flow.read_text():
    raise SystemExit('ERRO: documentação do fluxo atômico ausente/inválida.')
if 'FinalizeRfqSourcing' not in readme or 'FinalizeRfqSourcing' not in adr_index:
    raise SystemExit('ERRO: README/índice ADR não referenciam a nova capability.')
print('OK: README, guias, ADRs, fluxo atômico e CI refletem o desenho vigente.')
PY_CHECK
