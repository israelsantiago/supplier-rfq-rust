#!/usr/bin/env python3
"""Render every Mermaid block and print exact source input on failures."""
from __future__ import annotations

import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXCLUDED = {'.git', 'target', 'node_modules', '.venv', 'venv'}
OPEN_FENCE = re.compile(r'^[ \t]*```mermaid(?:[ \t].*)?$', re.IGNORECASE)
CLOSE_FENCE = re.compile(r'^[ \t]*```[ \t]*$')


def normalize_newlines(data: bytes) -> str:
    """Normalize only physical CRLF/CR/LF; keep Unicode separators literal."""
    return data.replace(b'\r\n', b'\n').replace(b'\r', b'\n').decode('utf-8')


def discover() -> list[tuple[str, int, str]]:
    found: list[tuple[str, int, str]] = []
    for path in sorted(ROOT.rglob('*')):
        if not path.is_file() or any(p in EXCLUDED for p in path.relative_to(ROOT).parts):
            continue
        rel = path.relative_to(ROOT).as_posix()
        try:
            if path.suffix.lower() == '.mmd':
                data = normalize_newlines(path.read_bytes())
                if data.strip():
                    found.append((rel, 1, data))
            elif path.suffix.lower() == '.md':
                # Use split('\n') deliberately; str.splitlines() also splits on
                # Unicode separators, which are not physical Markdown line endings.
                lines = normalize_newlines(path.read_bytes()).split('\n')
                i = 0
                while i < len(lines):
                    if not OPEN_FENCE.match(lines[i]):
                        i += 1
                        continue
                    start_line = i + 2
                    j = i + 1
                    while j < len(lines) and not CLOSE_FENCE.match(lines[j]):
                        j += 1
                    if j >= len(lines):
                        raise ValueError(f'Mermaid fence sem fechamento em {rel}:{i + 1}')
                    body = '\n'.join(lines[i + 1:j])
                    if body.strip():
                        found.append((rel, start_line, body))
                    i = j + 1
        except (UnicodeDecodeError, OSError, ValueError) as exc:
            raise SystemExit(f'ERRO ao ler {rel}: {exc}') from exc
    return found


def main() -> int:
    if not shutil.which('node'):
        print('ERRO: Node.js não encontrado.', file=sys.stderr)
        return 2
    mmdc = shutil.which('mmdc')
    npx = shutil.which('npx')
    if not mmdc and not npx:
        print('ERRO: nem mmdc nem npx encontrados.', file=sys.stderr)
        return 2

    diagrams = discover()
    if not diagrams:
        print('ERRO: nenhum diagrama Mermaid encontrado.', file=sys.stderr)
        return 2
    print(f'Encontrados {len(diagrams)} diagramas para validar.')
    for idx, (source, line, _) in enumerate(diagrams, 1):
        print(f'  {idx:04d}: {source}:{line}')
    if mmdc:
        command = [mmdc]
        print(f'Usando mmdc local: {mmdc}')
    else:
        command = [npx, '--yes', '--package', '@mermaid-js/mermaid-cli@11', 'mmdc']
        print('mmdc não instalado; usando npx com @mermaid-js/mermaid-cli@11.')

    failures = 0
    with tempfile.TemporaryDirectory(prefix='mermaid-audit-') as temp_name:
        temp = Path(temp_name)
        config = temp / 'puppeteer.json'
        config.write_text('{"args":["--no-sandbox","--disable-setuid-sandbox"]}\n', encoding='utf-8')
        for idx, (source, line, body) in enumerate(diagrams, 1):
            name = f'diagram-{idx:04d}'
            input_file = temp / f'{name}.mmd'
            output_file = temp / f'{name}.svg'
            # Write the exact content handed to mmdc as LF-delimited UTF-8.
            input_text = body.rstrip('\n') + '\n'
            input_file.write_bytes(input_text.encode('utf-8'))
            cmd = command + ['-i', str(input_file), '-o', str(output_file), '--puppeteerConfigFile', str(config)]
            result = subprocess.run(cmd, cwd=ROOT, text=True, capture_output=True)
            if result.returncode == 0 and output_file.is_file() and output_file.stat().st_size > 0:
                print(f'PASS: {source}:{line}')
                continue
            failures += 1
            print(f'FALHA: {source}:{line} (código {result.returncode})', file=sys.stderr)
            if result.stdout:
                print('--- stdout mmdc ---\n' + result.stdout[-12000:], file=sys.stderr)
            if result.stderr:
                print('--- stderr mmdc ---\n' + result.stderr[-16000:], file=sys.stderr)
            print('--- conteúdo EXATO do arquivo .mmd enviado a mmdc ---', file=sys.stderr)
            for lineno, diagram_line in enumerate(input_text.split('\n')[:-1], 1):
                print(f'{lineno:03d}: {diagram_line!r}', file=sys.stderr)
            print('--- fim do conteúdo .mmd ---', file=sys.stderr)

    if failures:
        print(f'FALHA: {failures} de {len(diagrams)} diagramas falharam.', file=sys.stderr)
        return 1
    print(f'PASS: todos os {len(diagrams)} diagramas foram renderizados por Mermaid CLI.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
