#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
WORKSPACE_ROOT=$(cd -- "${SCRIPT_DIR}/../.." && pwd)

usage() {
    cat <<'EOF'
Usage: version.sh [shell|json]

Outputs normalized release version metadata derived from the git workspace.
EOF
}

mode="${1:-shell}"

workspace_version=$(
    python3 - "$WORKSPACE_ROOT/Cargo.toml" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
capture = False
for line in text.splitlines():
    stripped = line.strip()
    if stripped == "[workspace.package]":
        capture = True
        continue
    if capture and stripped.startswith("["):
        break
    if capture and stripped.startswith("version"):
        value = stripped.split("=", 1)[1].strip().strip('"')
        print(value)
        break
PY
)

if [[ -z "${workspace_version}" ]]; then
    echo "failed to determine workspace version" >&2
    exit 1
fi

build_date="${BUILD_DATE:-$(date -u +%Y%m%d)}"
build_seq="${BUILD_SEQ:-}"

if [[ -z "${build_seq}" ]]; then
    if [[ -n "${CI:-}" ]]; then
        echo "BUILD_SEQ is required when CI is set" >&2
        exit 1
    fi
    build_seq="1"
fi

if [[ ! "${build_date}" =~ ^[0-9]{8}$ ]]; then
    echo "BUILD_DATE must be YYYYMMDD, got: ${build_date}" >&2
    exit 1
fi

if [[ ! "${build_seq}" =~ ^[0-9]+$ ]]; then
    echo "BUILD_SEQ must be numeric, got: ${build_seq}" >&2
    exit 1
fi

git_sha=$(git -C "${WORKSPACE_ROOT}" rev-parse --short=8 HEAD)
git_commit=$(git -C "${WORKSPACE_ROOT}" rev-parse HEAD)
git_dirty=false
if [[ -n "$(git -C "${WORKSPACE_ROOT}" status --porcelain)" ]]; then
    git_dirty=true
fi

printf -v python_seq '%02d' "${build_seq}"

logical_version="${workspace_version}-dev-${build_date}.${build_seq}-g${git_sha}"
deb_version="${workspace_version}~dev${build_date}.${build_seq}+g${git_sha}"
python_version="${workspace_version}.dev${build_date}${python_seq}"

if [[ "${git_dirty}" == "true" ]]; then
    logical_version="${logical_version}-modified"
    deb_version="${deb_version}.modified"
fi

deb_revision="1"
deb_full_version="${deb_version}-${deb_revision}"

host_arch=$(uname -m)
case "${host_arch}" in
    x86_64) deb_arch="amd64" ;;
    aarch64) deb_arch="arm64" ;;
    *)
        echo "unsupported host architecture: ${host_arch}" >&2
        exit 1
        ;;
esac

host_os=$(uname -s | tr '[:upper:]' '[:lower:]')
platform_id="${host_os}-${deb_arch}"

case "${mode}" in
    shell)
        cat <<EOF
WORKSPACE_ROOT='${WORKSPACE_ROOT}'
WORKSPACE_VERSION='${workspace_version}'
BUILD_DATE='${build_date}'
BUILD_SEQ='${build_seq}'
GIT_SHA='${git_sha}'
GIT_COMMIT='${git_commit}'
GIT_DIRTY='${git_dirty}'
LOGICAL_VERSION='${logical_version}'
DEB_VERSION='${deb_version}'
DEB_REVISION='${deb_revision}'
DEB_FULL_VERSION='${deb_full_version}'
PYTHON_VERSION='${python_version}'
HOST_ARCH='${host_arch}'
DEB_ARCH='${deb_arch}'
HOST_OS='${host_os}'
PLATFORM_ID='${platform_id}'
EOF
        ;;
    json)
        python3 - <<PY
import json
print(json.dumps({
  "workspace_root": ${WORKSPACE_ROOT@Q},
  "workspace_version": ${workspace_version@Q},
  "build_date": ${build_date@Q},
  "build_seq": ${build_seq@Q},
  "git_sha": ${git_sha@Q},
  "git_commit": ${git_commit@Q},
  "git_dirty": ${git_dirty@Q} == "true",
  "logical_version": ${logical_version@Q},
  "deb_version": ${deb_version@Q},
  "deb_revision": ${deb_revision@Q},
  "deb_full_version": ${deb_full_version@Q},
  "python_version": ${python_version@Q},
  "host_arch": ${host_arch@Q},
  "deb_arch": ${deb_arch@Q},
  "host_os": ${host_os@Q},
  "platform_id": ${platform_id@Q},
}, indent=2))
PY
        ;;
    -h|--help|help)
        usage
        ;;
    *)
        echo "unknown mode: ${mode}" >&2
        usage >&2
        exit 1
        ;;
esac
