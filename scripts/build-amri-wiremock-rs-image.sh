#!/usr/bin/env bash
set -Eeuo pipefail

REPO_INPUT="${1:-}"
if [[ -z "$REPO_INPUT" ]]; then
  echo "Uso: bash $0 /caminho/da/raiz/do/repositorio" >&2
  exit 64
fi
REPO="$(cd "$REPO_INPUT" && pwd -P)"
IMAGE="supplier-rfq-amri-wiremock-rs:0.6.5-stage5b2-v1"
EXPECTED_BRANCH="feat/amri-stage4-three-prepared-participants"
EXPECTED_HEAD="ae25951ddfa360c57946dd0b1ad47ba629ac6225"
BASE_IMAGES=("rust:1.100.0-bookworm" "debian:bookworm-slim")
PULL_RETRIES="${AMRI_DOCKER_PULL_RETRIES:-3}"
PULL_INITIAL_DELAY="${AMRI_DOCKER_PULL_INITIAL_DELAY_SECONDS:-3}"
MIN_FREE_GIB="${AMRI_DOCKER_MIN_FREE_GIB:-4}"

command -v git >/dev/null 2>&1 || { echo "FAIL: git ausente" >&2; exit 127; }
command -v docker >/dev/null 2>&1 || { echo "FAIL: Docker CLI ausente" >&2; exit 127; }
command -v cargo >/dev/null 2>&1 || { echo "FAIL: cargo ausente" >&2; exit 127; }
command -v python3 >/dev/null 2>&1 || { echo "FAIL: python3 ausente" >&2; exit 127; }
command -v sha256sum >/dev/null 2>&1 || { echo "FAIL: sha256sum ausente" >&2; exit 127; }

ROOT="$(git -C "$REPO" rev-parse --show-toplevel)"
[[ "$ROOT" == "$REPO" ]] || { echo "FAIL: caminho informado não é a raiz Git" >&2; exit 1; }
BRANCH="$(git -C "$REPO" branch --show-current)"
HEAD="$(git -C "$REPO" rev-parse HEAD)"
ORIGIN="$(git -C "$REPO" remote get-url origin)"
REMOTE_HEAD="$(git -C "$REPO" ls-remote --heads origin "refs/heads/$BRANCH" | awk 'NR==1 {print $1}')"

normalize_origin() {
  python3 - "$1" <<'PY_ORIGIN'
import sys
v=sys.argv[1].strip()
if v.startswith("git@") and ":" in v:
    host,path=v[4:].split(":",1); v=f"{host}/{path}"
else:
    for prefix in ("ssh://git@", "https://", "http://", "ssh://", "git@"):
        if v.startswith(prefix): v=v[len(prefix):]; break
v=v.rstrip("/")
if v.endswith(".git"): v=v[:-4]
print(v.lower())
PY_ORIGIN
}

ORIGIN_NORMALIZED="$(normalize_origin "$ORIGIN")"
printf '%s\n' \
  "REPOSITORY=$REPO" \
  "BRANCH=$BRANCH" \
  "HEAD=$HEAD" \
  "ORIGIN=$ORIGIN" \
  "REMOTE_HEAD=$REMOTE_HEAD" \
  "ORIGIN_NORMALIZED=$ORIGIN_NORMALIZED" \
  "WIREMOCK_IMAGE=$IMAGE"

[[ "$BRANCH" == "$EXPECTED_BRANCH" ]] || { echo "FAIL: branch inesperada: $BRANCH" >&2; exit 1; }
[[ "$HEAD" == "$EXPECTED_HEAD" ]] || { echo "FAIL: HEAD inesperado: $HEAD" >&2; exit 1; }
[[ "$REMOTE_HEAD" == "$EXPECTED_HEAD" ]] || { echo "FAIL: remote HEAD divergiu: $REMOTE_HEAD" >&2; exit 1; }
[[ "$ORIGIN_NORMALIZED" == "github.com/israelsantiago/supplier-rfq-rust" ]] || {
  echo "FAIL: origin inesperado: $ORIGIN" >&2; exit 1;
}
[[ "$PULL_RETRIES" =~ ^[1-5]$ ]] || { echo "FAIL: AMRI_DOCKER_PULL_RETRIES deve estar entre 1 e 5" >&2; exit 64; }
[[ "$PULL_INITIAL_DELAY" =~ ^[1-9][0-9]?$ ]] || { echo "FAIL: delay de pull inválido" >&2; exit 64; }
[[ "$MIN_FREE_GIB" =~ ^[1-9][0-9]?$ ]] || { echo "FAIL: AMRI_DOCKER_MIN_FREE_GIB deve ser inteiro entre 1 e 99" >&2; exit 64; }

for required in \
  "$REPO/tests/wiremock-rs/Dockerfile" \
  "$REPO/Cargo.lock" \
  "$REPO/Cargo.toml" \
  "$REPO/rust-toolchain.toml" \
  "$REPO/src" \
  "$REPO/tests/wiremock-rs/Cargo.toml" \
  "$REPO/tests/wiremock-rs/src/main.rs" \
  "$REPO/tests/acceptance_atomic_sourcing.rs" \
  "$REPO/tests/external_provider_contracts.rs"; do
  [[ -e "$required" ]] || { echo "FAIL: entrada necessária ausente: $required" >&2; exit 1; }
done

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_DIR="$HOME/Downloads/amri-stage5b2-wiremock-build-$STAMP"
mkdir -p "$RUN_DIR"
LOG="$RUN_DIR/build.log"
exec > >(tee -a "$LOG") 2>&1

printf 'BUILD_RUN_DIR=%s\nBUILD_LOG=%s\n' "$RUN_DIR" "$LOG"

# Always use the Docker Engine installed for the Ubuntu host. Do not inherit
# whichever Docker context happens to be active in the caller's shell.
DOCKER_CONTEXT="default"
DOCKER_ENDPOINT="$(env -u DOCKER_CONTEXT -u DOCKER_HOST -u BUILDX_BUILDER docker context inspect default --format '{{(index .Endpoints "docker").Host}}')"
case "$DOCKER_ENDPOINT" in
  unix:///var/run/docker.sock|unix:///run/docker.sock) ;;
  *)
    echo "FAIL: contexto Docker 'default' não aponta para o socket padrão do Ubuntu: $DOCKER_ENDPOINT" >&2
    echo "INFO: esperado unix:///var/run/docker.sock (ou unix:///run/docker.sock); nenhum outro daemon será usado." >&2
    exit 1
    ;;
esac
DOCKER_SOCKET="${DOCKER_ENDPOINT#unix://}"
[[ -S "$DOCKER_SOCKET" ]] || {
  echo "FAIL: socket do Docker Engine padrão não está disponível: $DOCKER_SOCKET" >&2
  echo "INFO: verifique/inicie docker.service e docker.socket." >&2
  exit 1
}
run_docker() {
  env -u DOCKER_CONTEXT -u DOCKER_HOST -u BUILDX_BUILDER docker --context default "$@"
}
run_docker info >/dev/null || {
  echo "FAIL: Docker Engine padrão indisponível em $DOCKER_ENDPOINT" >&2; exit 1;
}
DOCKER_ROOT_DIR="$(run_docker info --format '{{.DockerRootDir}}')"
STORAGE_DRIVER="$(run_docker info --format '{{.Driver}}')"
DRIVER_STATUS="$(run_docker info --format '{{json .DriverStatus}}')"
SERVER_VERSION="$(run_docker info --format '{{.ServerVersion}}')"
printf '%s\n' \
  "DOCKER_CONTEXT=$DOCKER_CONTEXT" \
  "DOCKER_ENDPOINT=$DOCKER_ENDPOINT" \
  "DOCKER_SERVER_VERSION=$SERVER_VERSION" \
  "DOCKER_ROOT_DIR=$DOCKER_ROOT_DIR" \
  "DOCKER_STORAGE_DRIVER=$STORAGE_DRIVER" \
  "DOCKER_DRIVER_STATUS=$DRIVER_STATUS" \
  "BUILDX_BUILDER_ENV_WAS_SET=$([[ -n "${BUILDX_BUILDER+x}" ]] && echo YES || echo NO)"

# Local df is valid only when the active Docker endpoint is a local Unix socket.
# For a remote Engine, report Docker's accounting instead of inspecting the wrong host.
STORAGE_PATHS=()
case "$DOCKER_ENDPOINT" in
  unix://*)
    [[ -d "$DOCKER_ROOT_DIR" ]] || { echo "FAIL: DockerRootDir não existe no host local: $DOCKER_ROOT_DIR" >&2; exit 1; }
    STORAGE_PATHS+=("$DOCKER_ROOT_DIR")
    if grep -qi 'containerd\.snapshotter' <<<"$DRIVER_STATUS"; then
      CONTAINERD_ROOT="/var/lib/containerd"
      if [[ -r /etc/containerd/config.toml ]]; then
        CONFIG_ROOT="$(python3 - <<'PY_CONTAINERD'
from pathlib import Path
import re
text=Path('/etc/containerd/config.toml').read_text(encoding='utf-8')
m=re.search(r'(?m)^\s*root\s*=\s*["\']([^"\']+)["\']', text)
if m: print(m.group(1))
PY_CONTAINERD
)"
        [[ -z "$CONFIG_ROOT" ]] || CONTAINERD_ROOT="$CONFIG_ROOT"
      fi
      [[ ! -d "$CONTAINERD_ROOT" ]] || STORAGE_PATHS+=("$CONTAINERD_ROOT")
    fi
    ;;
  *)
    echo "STORAGE_LOCAL_DF=SKIPPED_REMOTE_CONTEXT"
    echo "INFO: contexto remoto; df local não representa o armazenamento do Docker remoto."
    ;;
esac

MIN_FREE_BYTES=$(( MIN_FREE_GIB * 1024 * 1024 * 1024 ))
check_free_space() {
  local path="$1" available
  available="$(df -P -B1 -- "$path" | awk 'END {print $4}')"
  [[ "$available" =~ ^[0-9]+$ ]] || { echo "FAIL: não foi possível determinar espaço livre em $path" >&2; return 1; }
  printf 'STORAGE_PATH=%s\nSTORAGE_AVAILABLE_BYTES=%s\nSTORAGE_MINIMUM_BYTES=%s\n' "$path" "$available" "$MIN_FREE_BYTES"
  if (( available < MIN_FREE_BYTES )); then
    echo "FAIL: espaço livre insuficiente em $path; disponíveis=$available bytes; mínimo=$MIN_FREE_BYTES bytes" >&2
    echo "INFO: libere espaço ou ajuste conscientemente AMRI_DOCKER_MIN_FREE_GIB; nenhum prune será executado." >&2
    return 1
  fi
}
for storage_path in "${STORAGE_PATHS[@]}"; do check_free_space "$storage_path"; done

echo "========== DOCKER DISK USAGE BEFORE PULL =========="
run_docker system df -v

is_non_retryable_pull_error() {
  grep -Eqi 'no space left on device|unauthorized|authentication required|denied:|pull access denied|manifest unknown|manifest invalid|requested access to the resource is denied|not found: manifest' <<<"$1"
}
is_retryable_pull_error() {
  grep -Eqi 'i/o timeout|context deadline exceeded|connection reset|connection refused|temporary failure|TLS handshake timeout|unexpected EOF|connection timed out|502 Bad Gateway|503 Service Unavailable|504 Gateway Timeout|too many requests|429 Too Many Requests|net/http: request canceled|dial tcp|network is unreachable' <<<"$1"
}

for image in "${BASE_IMAGES[@]}"; do
  echo "========== BASE IMAGE: $image =========="
  pull_ok=0
  attempt=1
  while (( attempt <= PULL_RETRIES )); do
    echo "DOCKER_PULL_IMAGE=$image ATTEMPT=$attempt/$PULL_RETRIES"
    if output="$(run_docker pull "$image" 2>&1)"; then
      printf '%s\n' "$output"
      pull_ok=1
      break
    else
      rc=$?
      printf '%s\n' "$output"
      echo "DOCKER_PULL_EXIT_CODE=$rc"
      if is_non_retryable_pull_error "$output"; then
        echo "PULL_RETRY=NO_REASON=NON_RETRYABLE" >&2
        break
      fi
      if ! is_retryable_pull_error "$output"; then
        echo "PULL_RETRY=NO_REASON=UNCLASSIFIED_FAILURE" >&2
        break
      fi
      if (( attempt < PULL_RETRIES )); then
        delay=$(( PULL_INITIAL_DELAY * attempt ))
        echo "PULL_RETRY=YES DELAY_SECONDS=$delay"
        sleep "$delay"
      fi
    fi
    attempt=$((attempt + 1))
  done
  (( pull_ok == 1 )) || { echo "FAIL: não foi possível obter imagem-base: $image" >&2; exit 1; }
  image_id="$(run_docker image inspect --format '{{.Id}}' "$image")"
  printf 'BASE_IMAGE_READY=%s\nBASE_IMAGE_ID=%s\n' "$image" "$image_id"
  for storage_path in "${STORAGE_PATHS[@]}"; do check_free_space "$storage_path"; done
done

echo "========== DOCKER DISK USAGE AFTER PULL =========="
run_docker system df -v

echo "========== HOST CARGO CHECK =========="
cargo check --manifest-path "$REPO/Cargo.toml" -p amri-stage5b2-wiremock-rs --locked

# Build context is intentionally curated: never send .git, target/, backups,
# Downloads, logs or unrelated repository content to the Docker daemon.
BUILD_CONTEXT="$RUN_DIR/context"
[[ ! -e "$BUILD_CONTEXT" ]] || { echo "FAIL: contexto temporário já existe: $BUILD_CONTEXT" >&2; exit 1; }
mkdir -p "$BUILD_CONTEXT/tests"
cp -a "$REPO/Cargo.toml" "$REPO/Cargo.lock" "$REPO/rust-toolchain.toml" "$BUILD_CONTEXT/"
cp -a "$REPO/src" "$BUILD_CONTEXT/src"
# Copy the CONTENTS of tests/ into the context's tests/ directory.
# Do not copy the tests/ directory itself, which would create tests/tests/ and
# break the Cargo workspace member path tests/wiremock-rs.
cp -a "$REPO/tests/." "$BUILD_CONTEXT/tests/"
cp -a "$REPO/tests/wiremock-rs/Dockerfile" "$BUILD_CONTEXT/Dockerfile"

# Fail before invoking Docker if the curated context does not preserve the
# workspace member manifest at the exact relative path expected by Cargo.
[[ -f "$BUILD_CONTEXT/tests/wiremock-rs/Cargo.toml" ]] || {
  echo "FAIL: contexto Docker incompleto: tests/wiremock-rs/Cargo.toml ausente" >&2
  echo "INFO: verifique a cópia do conteúdo de tests/ para o contexto" >&2
  exit 1
}
[[ -f "$BUILD_CONTEXT/tests/wiremock-rs/src/main.rs" ]] || {
  echo "FAIL: contexto Docker incompleto: tests/wiremock-rs/src/main.rs ausente" >&2
  exit 1
}
if [[ -e "$BUILD_CONTEXT/tests/tests/wiremock-rs/Cargo.toml" ]]; then
  echo "FAIL: contexto Docker possui aninhamento inválido tests/tests/wiremock-rs" >&2
  exit 1
fi
printf 'BUILD_CONTEXT_WORKSPACE_MEMBER=tests/wiremock-rs/Cargo.toml\n'

export BUILD_CONTEXT
python3 - <<'PY_CONTEXT'
import hashlib, json, os
from pathlib import Path
root=Path(os.environ['BUILD_CONTEXT'])
files=[]
for p in sorted(x for x in root.rglob('*') if x.is_file()):
    raw=p.read_bytes()
    files.append({'path':p.relative_to(root).as_posix(),'size_bytes':len(raw),
                  'sha256':hashlib.sha256(raw).hexdigest()})
if not files:
    raise SystemExit('FAIL: contexto Docker vazio')
paths={item['path'] for item in files}
required={'Cargo.toml','Cargo.lock','rust-toolchain.toml','Dockerfile',
          'tests/wiremock-rs/Cargo.toml','tests/wiremock-rs/src/main.rs'}
missing=sorted(required-paths)
if missing:
    raise SystemExit('FAIL: contexto Docker não preserva caminhos requeridos: '+', '.join(missing))
if any(path.startswith('tests/tests/wiremock-rs/') for path in paths):
    raise SystemExit('FAIL: contexto Docker contém árvore tests/tests/wiremock-rs inválida')
manifest={'file_count':len(files),'total_bytes':sum(x['size_bytes'] for x in files),'files':files}
(root.parent/'build-context-manifest.json').write_text(json.dumps(manifest,indent=2,sort_keys=True)+'\n',encoding='utf-8')
print(f"BUILD_CONTEXT_FILES={manifest['file_count']}")
print(f"BUILD_CONTEXT_BYTES={manifest['total_bytes']}")
for item in files: print(f"BUILD_CONTEXT_FILE={item['path']} SHA256={item['sha256']}")
PY_CONTEXT

CONTEXT_BYTES="$(du -sb "$BUILD_CONTEXT" | awk '{print $1}')"
printf 'BUILD_CONTEXT_PATH=%s\nBUILD_CONTEXT_DISK_BYTES=%s\n' "$BUILD_CONTEXT" "$CONTEXT_BYTES"
HOST_CONTEXT_FREE="$(df -P -B1 -- "$BUILD_CONTEXT" | awk 'END {print $4}')"
[[ "$HOST_CONTEXT_FREE" =~ ^[0-9]+$ ]] || { echo "FAIL: espaço livre do filesystem do contexto não pôde ser verificado" >&2; exit 1; }
HOST_CONTEXT_MIN=$((512 * 1024 * 1024))
printf 'HOST_CONTEXT_FREE_BYTES=%s\nHOST_CONTEXT_MINIMUM_BYTES=%s\n' "$HOST_CONTEXT_FREE" "$HOST_CONTEXT_MIN"
(( HOST_CONTEXT_FREE >= HOST_CONTEXT_MIN )) || { echo "FAIL: menos de 512 MiB livres no filesystem do contexto temporário" >&2; exit 1; }
for storage_path in "${STORAGE_PATHS[@]}"; do check_free_space "$storage_path"; done

echo "========== BUILD WIREMOCK-RS WITH UBUNTU DOCKER ENGINE =========="
printf 'BUILD_COMMAND=env -u DOCKER_CONTEXT -u DOCKER_HOST -u BUILDX_BUILDER DOCKER_BUILDKIT=1 docker --context default build --pull=false --file %q --tag %q %q\n' \
  "$BUILD_CONTEXT/Dockerfile" "$IMAGE" "$BUILD_CONTEXT"
# Every pull, inspection and build is pinned to the Ubuntu host Docker Engine.
# No global Docker context, Buildx selection, daemon configuration or prune is changed.
env -u DOCKER_CONTEXT -u DOCKER_HOST -u BUILDX_BUILDER DOCKER_BUILDKIT=1 \
  docker --context default build \
    --pull=false \
    --file "$BUILD_CONTEXT/Dockerfile" \
    --tag "$IMAGE" \
    "$BUILD_CONTEXT"

IMAGE_ID="$(run_docker image inspect --format '{{.Id}}' "$IMAGE")"
IMAGE_SIZE="$(run_docker image inspect --format '{{.Size}}' "$IMAGE")"
printf 'PASS: imagem WireMock-RS construída e carregada no image store do Docker Engine.\nIMAGE=%s\nIMAGE_ID=%s\nIMAGE_SIZE_BYTES=%s\n' \
  "$IMAGE" "$IMAGE_ID" "$IMAGE_SIZE"

export REPO IMAGE IMAGE_ID IMAGE_SIZE DOCKER_CONTEXT DOCKER_ENDPOINT DOCKER_ROOT_DIR STORAGE_DRIVER SERVER_VERSION LOG RUN_DIR BUILD_CONTEXT CONTEXT_BYTES
export BUILD_SCRIPT_SHA256="$(sha256sum "$REPO/scripts/build-amri-wiremock-rs-image.sh" | awk '{print $1}')"
export LOG_SHA256="$(sha256sum "$LOG" | awk '{print $1}')"
export CONTEXT_MANIFEST_SHA256="$(sha256sum "$RUN_DIR/build-context-manifest.json" | awk '{print $1}')"
python3 - <<'PY_RECEIPT'
import hashlib, json, os
from datetime import datetime, timezone
from pathlib import Path
out=Path(os.environ['RUN_DIR'])/'docker-image-build-receipt.json'
data={
  'receipt_type':'WireMockDockerImageBuild', 'result':'PASS',
  'created_at_utc':datetime.now(timezone.utc).isoformat(),
  'repository':os.environ['REPO'], 'image':os.environ['IMAGE'],
  'image_id':os.environ['IMAGE_ID'], 'image_size_bytes':int(os.environ['IMAGE_SIZE']),
  'docker_context':os.environ['DOCKER_CONTEXT'], 'docker_endpoint':os.environ['DOCKER_ENDPOINT'],
  'docker_root_dir':os.environ['DOCKER_ROOT_DIR'], 'storage_driver':os.environ['STORAGE_DRIVER'],
  'docker_server_version':os.environ['SERVER_VERSION'],
  'build_script_sha256':os.environ['BUILD_SCRIPT_SHA256'], 'build_log':os.environ['LOG'],
  'build_log_sha256':os.environ['LOG_SHA256'],
  'build_context':os.environ['BUILD_CONTEXT'], 'build_context_disk_bytes':int(os.environ['CONTEXT_BYTES']),
  'build_context_manifest':str(Path(os.environ['RUN_DIR'])/'build-context-manifest.json'),
  'build_context_manifest_sha256':os.environ['CONTEXT_MANIFEST_SHA256'],
  'builder_selection':'DOCKER_ENGINE_INTEGRATED_DEFAULT',
  'build_command':'env -u DOCKER_CONTEXT -u DOCKER_HOST -u BUILDX_BUILDER DOCKER_BUILDKIT=1 docker --context default build --pull=false --file <context-Dockerfile> <curated-context>',
  'docker_context_mutation':'NOT_EXECUTED', 'buildx_global_selection_mutation':'NOT_EXECUTED',
  'docker_prune':'NOT_EXECUTED'
}
raw=(json.dumps(data,indent=2,ensure_ascii=False,sort_keys=True)+'\n').encode()
tmp=out.with_suffix('.json.tmp'); tmp.write_bytes(raw); os.replace(tmp,out)
digest=hashlib.sha256(raw).hexdigest()
out.with_suffix('.json.sha256').write_text(f'{digest}  {out.name}\n',encoding='utf-8')
print(f'DOCKER_IMAGE_BUILD_RECEIPT={out}')
print(f'DOCKER_IMAGE_BUILD_RECEIPT_SHA256={digest}')
PY_RECEIPT
printf 'BUILD_SCRIPT_SHA256=%s\n' "$BUILD_SCRIPT_SHA256"
printf 'BUILD_LOG=%s\nBUILD_LOG_SHA256=%s\n' "$LOG" "$LOG_SHA256"
printf 'DOCKER_IMAGE_BUILD=PASS\n'
