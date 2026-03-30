#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
PACKAGING_ROOT=$(cd -- "${SCRIPT_DIR}/.." && pwd)
TEMPLATE_DIR="${PACKAGING_ROOT}/templates"

usage() {
    cat <<'EOF'
Usage:
  package-deb.sh --package-name NAME --deb-version VERSION --arch ARCH \
    --description TEXT --manifest PATH --generated-dir PATH --output-dir PATH \
    [--depends DEPENDS] [--maintainer MAINTAINER] [--workspace-root PATH] \
    [--rust-release-dir PATH] [--deb-host-multiarch TRIPLET]
EOF
}

package_name=""
deb_version=""
arch=""
description=""
depends=""
maintainer="zenoh-plugin-grpc maintainers"
manifest=""
generated_dir=""
output_dir=""
workspace_root=""
rust_release_dir=""
deb_host_multiarch=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --package-name) package_name="$2"; shift 2 ;;
        --deb-version) deb_version="$2"; shift 2 ;;
        --arch) arch="$2"; shift 2 ;;
        --description) description="$2"; shift 2 ;;
        --depends) depends="$2"; shift 2 ;;
        --maintainer) maintainer="$2"; shift 2 ;;
        --manifest) manifest="$2"; shift 2 ;;
        --generated-dir) generated_dir="$2"; shift 2 ;;
        --output-dir) output_dir="$2"; shift 2 ;;
        --workspace-root) workspace_root="$2"; shift 2 ;;
        --rust-release-dir) rust_release_dir="$2"; shift 2 ;;
        --deb-host-multiarch) deb_host_multiarch="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

for required in package_name deb_version arch description manifest generated_dir output_dir workspace_root rust_release_dir deb_host_multiarch; do
    if [[ -z "${!required}" ]]; then
        echo "missing required argument: ${required}" >&2
        usage >&2
        exit 1
    fi
done

if [[ ! -f "${manifest}" ]]; then
    echo "manifest not found: ${manifest}" >&2
    exit 1
fi

stage_dir=$(mktemp -d "${TMPDIR:-/tmp}/${package_name}.stage.XXXXXX")
chmod 0755 "${stage_dir}"
cleanup() {
    rm -rf "${stage_dir}"
}
trap cleanup EXIT

subst() {
    local value="$1"
    value=${value//@WORKSPACE_ROOT@/${workspace_root}}
    value=${value//@RUST_RELEASE_DIR@/${rust_release_dir}}
    value=${value//@GENERATED_DIR@/${generated_dir}}
    value=${value//@DEB_HOST_MULTIARCH@/${deb_host_multiarch}}
    printf '%s' "${value}"
}

while IFS='|' read -r kind raw_source raw_dest raw_mode; do
    [[ -z "${kind}" ]] && continue
    [[ "${kind}" =~ ^# ]] && continue

    source_path=$(subst "${raw_source}")
    dest_path=$(subst "${raw_dest}")
    mode=$(subst "${raw_mode}")

    case "${kind}" in
        file)
            if [[ ! -f "${source_path}" ]]; then
                echo "manifest file source missing: ${source_path}" >&2
                exit 1
            fi
            install -D -m "${mode}" "${source_path}" "${stage_dir}${dest_path}"
            ;;
        dir)
            if [[ ! -d "${source_path}" ]]; then
                echo "manifest directory source missing: ${source_path}" >&2
                exit 1
            fi
            mkdir -p "${stage_dir}${dest_path}"
            cp -a "${source_path}/." "${stage_dir}${dest_path}/"
            chmod "${mode}" "${stage_dir}${dest_path}"
            ;;
        *)
            echo "unsupported manifest entry kind: ${kind}" >&2
            exit 1
            ;;
    esac
done < "${manifest}"

optional_depends=""
if [[ -n "${depends}" ]]; then
    optional_depends="Depends: ${depends}"
fi

mkdir -p "${stage_dir}/DEBIAN" "${output_dir}"

python3 - "${TEMPLATE_DIR}/control.in" "${stage_dir}/DEBIAN/control" <<PY
from pathlib import Path
import sys

template_path = Path(sys.argv[1])
output_path = Path(sys.argv[2])
text = template_path.read_text(encoding="utf-8")
mapping = {
    "@PACKAGE_NAME@": ${package_name@Q},
    "@DEB_VERSION@": ${deb_version@Q},
    "@ARCH@": ${arch@Q},
    "@MAINTAINER@": ${maintainer@Q},
    "@DESCRIPTION@": ${description@Q},
    "@OPTIONAL_DEPENDS@": ${optional_depends@Q},
}
for key, value in mapping.items():
    text = text.replace(key, value)
text = "\n".join(line for line in text.splitlines() if line.strip()) + "\n"
output_path.write_text(text, encoding="utf-8")
PY

chmod 0644 "${stage_dir}/DEBIAN/control"

package_path="${output_dir}/${package_name}_${deb_version}_${arch}.deb"
dpkg-deb --root-owner-group --build "${stage_dir}" "${package_path}" >/dev/null
printf '%s\n' "${package_path}"
