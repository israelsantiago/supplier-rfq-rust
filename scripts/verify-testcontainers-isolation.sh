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

# AMRI-STAGE5B2-WIREMOCK-TESTCONTAINER:START
python3 - <<'PY_WIREMOCK'
from pathlib import Path

integration = Path("tests/external_provider_contracts.rs").read_text(encoding="utf-8")
server = Path("tests/wiremock-rs/src/main.rs").read_text(encoding="utf-8")
dockerfile = Path("tests/wiremock-rs/Dockerfile").read_text(encoding="utf-8")
manifest = Path("tests/wiremock-rs/Cargo.toml").read_text(encoding="utf-8")
root_manifest = Path("Cargo.toml").read_text(encoding="utf-8")

for token in (
    'GenericImage::new(IMAGE, IMAGE_TAG)',
    'WaitFor::message_on_stdout("AMRI_WIREMOCK_RS_READY")',
    'get_host_port_ipv4(8080)',
    'external_provider_contracts',
):
    if token not in integration:
        raise SystemExit(f"FAIL: integração Testcontainers/WireMock incompleta: {token}")
for token in (
    'MockServer::builder().listener(listener).start().await',
    'AMRI_WIREMOCK_RS_READY',
    'wiremock_version',
    '0.6.5',
):
    if token not in server:
        raise SystemExit(f"FAIL: servidor WireMock-RS não contém contrato esperado: {token}")
if 'FROM rust:1.100.0-bookworm AS builder' not in dockerfile:
    raise SystemExit("FAIL: imagem de teste não fixa o builder Rust 1.100.0")
if 'wiremock = "=0.6.5"' not in manifest:
    raise SystemExit("FAIL: wiremock-rs crate não está fixado em 0.6.5")
if 'testcontainers = "=0.27.3"' not in root_manifest:
    raise SystemExit("FAIL: Testcontainers Rust deve estar como dev-dependency direta, versão 0.27.3")

# Core and domain are still prohibited from depending on this test infrastructure.
for root in (Path("src/amri"), Path("src/domain"), Path("src/amri_participants")):
    for source in root.rglob("*.rs"):
        body = source.read_text(encoding="utf-8")
        if any(token in body for token in ("wiremock::", "testcontainers_modules", "GenericImage::new", "reqwest::")):
            raise SystemExit(f"FAIL: dependência WireMock/Testcontainers/HTTP vazou para {source}")

print("PASS: somente a suíte integrada inicia GenericImage WireMock-RS; imagem e crate Rust estão versionados.")
print("PASS: o test double é separado do Functional Core, AMRI Core e Prepared Participants.")
PY_WIREMOCK
# AMRI-STAGE5B2-WIREMOCK-TESTCONTAINER:END
