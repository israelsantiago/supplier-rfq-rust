#!/usr/bin/env bash
set -Eeuo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

fixture="tests/common/postgres.rs"
[[ -f "$fixture" ]] || { echo "FALHA: fixture compartilhado ausente: $fixture" >&2; exit 1; }

python3 - <<'PY'
from pathlib import Path

fixture = Path('tests/common/postgres.rs').read_text(encoding='utf-8')
for forbidden in ('unsafe', 'atexit', 'std::future::pending', 'shutdown_shared_postgres'):
    if forbidden in fixture:
        raise SystemExit(f'FALHA: lifecycle do fixture não deve conter {forbidden!r}.')

required = (
    'OnceLock<tokio::sync::Mutex<Weak<SharedPostgres>>>',
    'async fn shared_postgres() -> Arc<SharedPostgres>',
    'Arc::downgrade(&shared)',
    'impl Drop for SharedPostgres',
    'shutdown_sender: mpsc::Sender<()>',
    'owner_thread: Option<JoinHandle<()>>',
    'let _ = self.shutdown_sender.send(())',
    'handle.join()',
    '.rm()',
    'CREATE_DATABASE_PERMITS: Semaphore = Semaphore::const_new(1)',
    'MAX_CONCURRENT_DATABASE_TESTS: usize = 4',
    '_shared_postgres: Arc<SharedPostgres>',
    'pub async fn create_isolated_database()',
    'CREATE DATABASE',
    'FIXTURE_LABEL_VALUE: &str = "shared-postgres-v1"',
)
for token in required:
    if token not in fixture:
        raise SystemExit(f'FALHA: contrato RAII/isolation ausente no fixture: {token!r}.')

for filename in ('tests/acceptance_api.rs', 'tests/acceptance_atomic_sourcing.rs'):
    source = Path(filename).read_text(encoding='utf-8')
    if '#[path = "common/postgres.rs"]' not in source or 'create_isolated_database().await' not in source:
        raise SystemExit(f'FALHA: {filename} não usa o fixture isolado.')
    for pattern in ('Postgres::default()', 'ContainerAsync<Postgres>', 'ImageExt, runners::AsyncRunner'):
        if pattern in source:
            raise SystemExit(f'FALHA: {filename} ainda gerencia Testcontainers diretamente ({pattern}).')

print('OK: o cache contém Weak; cada fixture mantém Arc e libera o container via Drop + join.')
print('OK: nenhuma utilização de unsafe/FFI/atexit no fixture de testes.')
print('OK: banco isolado por fixture; criação serializada; concorrência limitada a quatro.')
print('OK: somente as suítes de integração esperadas usam o fixture; produção e Functional Core não são alterados.')
PY
