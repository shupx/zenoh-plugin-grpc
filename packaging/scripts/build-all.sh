#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
PACKAGING_ROOT=$(cd -- "${SCRIPT_DIR}/.." && pwd)
WORKSPACE_ROOT=$(cd -- "${PACKAGING_ROOT}/.." && pwd)

usage() {
    cat <<'EOF'
Usage:
  build-all.sh [--date YYYYMMDD] [--seq N] [--only LIST] [--out-dir DIR]

Examples:
  build-all.sh --date 20260330 --seq 1
  build-all.sh --date 20260330 --seq 1 --only bridge,plugin,python
EOF
}

only=""
custom_out_dir=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --date) export BUILD_DATE="$2"; shift 2 ;;
        --seq) export BUILD_SEQ="$2"; shift 2 ;;
        --only) only="$2"; shift 2 ;;
        --out-dir) custom_out_dir="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

eval "$("${PACKAGING_ROOT}/scripts/version.sh" shell)"

OUT_DIR="${custom_out_dir:-${WORKSPACE_ROOT}/dist/${PLATFORM_ID}}"
WORK_DIR="${WORKSPACE_ROOT}/packaging/.work/${PLATFORM_ID}"
GENERATED_DIR="${WORK_DIR}/generated"
mkdir -p "${OUT_DIR}" "${GENERATED_DIR}"

RUST_BUILD_ARGS=(--release)
if [[ -n "${RUST_TARGET:-}" ]]; then
    RUST_BUILD_ARGS+=(--target "${RUST_TARGET}")
    RUST_RELEASE_DIR="${WORKSPACE_ROOT}/target/${RUST_TARGET}/release"
else
    RUST_RELEASE_DIR="${WORKSPACE_ROOT}/target/release"
fi

DEB_HOST_MULTIARCH=$(dpkg-architecture -a"${DEB_ARCH}" -qDEB_HOST_MULTIARCH)

selected() {
    local item="$1"
    if [[ -z "${only}" ]]; then
        return 0
    fi
    case ",${only}," in
        *,"${item}",*) return 0 ;;
        *) return 1 ;;
    esac
}

write_changelog() {
    local pkg="$1"
    local doc_dir="${GENERATED_DIR}/docs/${pkg}"
    mkdir -p "${doc_dir}"
    cat <<EOF > "${doc_dir}/changelog.Debian"
${pkg} (${DEB_FULL_VERSION}) unstable; urgency=medium

  * Automated development build from ${GIT_COMMIT}
  * Canonical version: ${LOGICAL_VERSION}

 -- zenoh-plugin-grpc maintainers  $(date -Ru)
EOF
    gzip -n -f "${doc_dir}/changelog.Debian"
}

render_cmake_files() {
    local cmake_dir="${GENERATED_DIR}/cmake/zenohgrpcxx"
    mkdir -p "${cmake_dir}"
    cp "${PACKAGING_ROOT}/templates/cmake/zenohgrpcxxConfig.cmake.in" "${cmake_dir}/zenohgrpcxxConfig.cmake"
    cp "${PACKAGING_ROOT}/templates/cmake/zenohgrpcxxTargets.cmake.in" "${cmake_dir}/zenohgrpcxxTargets.cmake"
    python3 - "${PACKAGING_ROOT}/templates/cmake/zenohgrpcxxConfigVersion.cmake.in" "${cmake_dir}/zenohgrpcxxConfigVersion.cmake" <<PY
from pathlib import Path
import sys
text = Path(sys.argv[1]).read_text(encoding="utf-8")
text = text.replace("@PACKAGE_VERSION@", ${WORKSPACE_VERSION@Q})
Path(sys.argv[2]).write_text(text, encoding="utf-8")
PY
}

artifact_paths=()
record_artifact() {
    artifact_paths+=("$1")
}

build_bridge() {
    cargo build -p zenoh-bridge-grpc "${RUST_BUILD_ARGS[@]}"
    write_changelog "zenoh-bridge-grpc"
    local deb
    deb=$("${PACKAGING_ROOT}/scripts/package-deb.sh" \
        --package-name "zenoh-bridge-grpc" \
        --deb-version "${DEB_FULL_VERSION}" \
        --arch "${DEB_ARCH}" \
        --description "Standalone Zenoh gRPC bridge executable" \
        --manifest "${PACKAGING_ROOT}/templates/manifests/zenoh-bridge-grpc.manifest" \
        --generated-dir "${GENERATED_DIR}" \
        --output-dir "${OUT_DIR}" \
        --workspace-root "${WORKSPACE_ROOT}" \
        --rust-release-dir "${RUST_RELEASE_DIR}" \
        --deb-host-multiarch "${DEB_HOST_MULTIARCH}")
    record_artifact "${deb}"
}

build_plugin() {
    cargo build -p zenoh-plugin-grpc "${RUST_BUILD_ARGS[@]}"
    write_changelog "zenoh-plugin-grpc"
    local deb
    deb=$("${PACKAGING_ROOT}/scripts/package-deb.sh" \
        --package-name "zenoh-plugin-grpc" \
        --deb-version "${DEB_FULL_VERSION}" \
        --arch "${DEB_ARCH}" \
        --description "Dynamic Zenoh gRPC plugin library" \
        --manifest "${PACKAGING_ROOT}/templates/manifests/zenoh-plugin-grpc.manifest" \
        --generated-dir "${GENERATED_DIR}" \
        --output-dir "${OUT_DIR}" \
        --workspace-root "${WORKSPACE_ROOT}" \
        --rust-release-dir "${RUST_RELEASE_DIR}" \
        --deb-host-multiarch "${DEB_HOST_MULTIARCH}")
    record_artifact "${deb}"
}

build_client_rs() {
    write_changelog "zenoh-grpc-client-rs"
    local deb
    deb=$("${PACKAGING_ROOT}/scripts/package-deb.sh" \
        --package-name "zenoh-grpc-client-rs" \
        --deb-version "${DEB_FULL_VERSION}" \
        --arch "all" \
        --description "Rust source-development package for the Zenoh gRPC client SDK" \
        --manifest "${PACKAGING_ROOT}/templates/manifests/zenoh-grpc-client-rs.manifest" \
        --generated-dir "${GENERATED_DIR}" \
        --output-dir "${OUT_DIR}" \
        --workspace-root "${WORKSPACE_ROOT}" \
        --rust-release-dir "${RUST_RELEASE_DIR}" \
        --deb-host-multiarch "${DEB_HOST_MULTIARCH}")
    record_artifact "${deb}"
}

build_c() {
    cargo build -p zenoh-grpc-c "${RUST_BUILD_ARGS[@]}"
    write_changelog "zenoh-grpc-c"
    local deb
    deb=$("${PACKAGING_ROOT}/scripts/package-deb.sh" \
        --package-name "zenoh-grpc-c" \
        --deb-version "${DEB_FULL_VERSION}" \
        --arch "${DEB_ARCH}" \
        --description "C development package for the Zenoh gRPC client SDK" \
        --manifest "${PACKAGING_ROOT}/templates/manifests/zenoh-grpc-c.manifest" \
        --generated-dir "${GENERATED_DIR}" \
        --output-dir "${OUT_DIR}" \
        --workspace-root "${WORKSPACE_ROOT}" \
        --rust-release-dir "${RUST_RELEASE_DIR}" \
        --deb-host-multiarch "${DEB_HOST_MULTIARCH}")
    record_artifact "${deb}"
}

build_cpp() {
    render_cmake_files
    write_changelog "zenoh-grpc-cpp"
    local deb
    deb=$("${PACKAGING_ROOT}/scripts/package-deb.sh" \
        --package-name "zenoh-grpc-cpp" \
        --deb-version "${DEB_FULL_VERSION}" \
        --arch "all" \
        --description "C++ headers and CMake package files for the Zenoh gRPC client SDK" \
        --depends "zenoh-grpc-c (= ${DEB_FULL_VERSION})" \
        --manifest "${PACKAGING_ROOT}/templates/manifests/zenoh-grpc-cpp.manifest" \
        --generated-dir "${GENERATED_DIR}" \
        --output-dir "${OUT_DIR}" \
        --workspace-root "${WORKSPACE_ROOT}" \
        --rust-release-dir "${RUST_RELEASE_DIR}" \
        --deb-host-multiarch "${DEB_HOST_MULTIARCH}")
    record_artifact "${deb}"
}

build_python() {
    "${PACKAGING_ROOT}/scripts/build-python.sh" \
        --python-version "${PYTHON_VERSION}" \
        --out-dir "${OUT_DIR}" \
        --work-dir "${WORK_DIR}/python"

    shopt -s nullglob
    local file
    for file in "${OUT_DIR}"/zenoh_grpc-"${PYTHON_VERSION}"*; do
        record_artifact "${file}"
    done
    shopt -u nullglob
}

selected bridge && build_bridge
selected plugin && build_plugin
selected client-rs && build_client_rs
selected c && build_c
selected cpp && build_cpp
selected python && build_python

python3 - "${OUT_DIR}" "${artifact_paths[@]}" <<PY
import json
import sys
from pathlib import Path

out_dir = Path(sys.argv[1])
artifacts = [str(Path(p).resolve()) for p in sys.argv[2:]]
payload = {
    "platform": ${PLATFORM_ID@Q},
    "host_os": ${HOST_OS@Q},
    "host_arch": ${HOST_ARCH@Q},
    "deb_arch": ${DEB_ARCH@Q},
    "deb_host_multiarch": ${DEB_HOST_MULTIARCH@Q},
    "workspace_version": ${WORKSPACE_VERSION@Q},
    "logical_version": ${LOGICAL_VERSION@Q},
    "deb_version": ${DEB_FULL_VERSION@Q},
    "python_version": ${PYTHON_VERSION@Q},
    "build_date": ${BUILD_DATE@Q},
    "build_sequence": ${BUILD_SEQ@Q},
    "git_sha": ${GIT_SHA@Q},
    "git_commit": ${GIT_COMMIT@Q},
    "git_dirty": ${GIT_DIRTY@Q} == "true",
    "artifacts": artifacts,
}
(out_dir / "manifest.json").write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
(out_dir / "BUILD_INFO.txt").write_text(
    "\n".join([
        f"platform: {payload['platform']}",
        f"workspace_version: {payload['workspace_version']}",
        f"logical_version: {payload['logical_version']}",
        f"deb_version: {payload['deb_version']}",
        f"python_version: {payload['python_version']}",
        f"build_date: {payload['build_date']}",
        f"build_sequence: {payload['build_sequence']}",
        f"git_commit: {payload['git_commit']}",
        f"git_dirty: {payload['git_dirty']}",
        "artifacts:",
        *[f"  - {item}" for item in artifacts],
    ]) + "\n",
    encoding="utf-8",
)
PY

printf 'Artifacts written to %s\n' "${OUT_DIR}"
