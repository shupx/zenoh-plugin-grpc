#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
WORKSPACE_ROOT=$(cd -- "${SCRIPT_DIR}/../.." && pwd)

usage() {
    cat <<'EOF'
Usage:
  build-python.sh --python-version VERSION --out-dir DIR [--work-dir DIR]
EOF
}

python_version=""
out_dir=""
work_dir=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --python-version) python_version="$2"; shift 2 ;;
        --out-dir) out_dir="$2"; shift 2 ;;
        --work-dir) work_dir="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

if [[ -z "${python_version}" || -z "${out_dir}" ]]; then
    usage >&2
    exit 1
fi

if [[ -z "${work_dir}" ]]; then
    work_dir=$(mktemp -d "${TMPDIR:-/tmp}/zenoh-grpc-python.XXXXXX")
    trap 'rm -rf "${work_dir}"' EXIT
else
    rm -rf "${work_dir}"
    mkdir -p "${work_dir}"
fi

mkdir -p "${out_dir}" "${work_dir}/zenoh-grpc-client-sdk"

cp -a "${WORKSPACE_ROOT}/Cargo.toml" "${work_dir}/Cargo.toml"
cp -a "${WORKSPACE_ROOT}/Cargo.lock" "${work_dir}/Cargo.lock"
cp -a "${WORKSPACE_ROOT}/rust-toolchain.toml" "${work_dir}/rust-toolchain.toml"
cp -a "${WORKSPACE_ROOT}/LICENSE" "${work_dir}/LICENSE"
cp -a "${WORKSPACE_ROOT}/zenoh-grpc-proto" "${work_dir}/zenoh-grpc-proto"
cp -a "${WORKSPACE_ROOT}/zenoh-grpc-client-sdk/zenoh-grpc-client-rs" "${work_dir}/zenoh-grpc-client-sdk/zenoh-grpc-client-rs"
cp -a "${WORKSPACE_ROOT}/zenoh-grpc-client-sdk/zenoh-grpc-python" "${work_dir}/zenoh-grpc-client-sdk/zenoh-grpc-python"
cp -a "${WORKSPACE_ROOT}/LICENSE" "${work_dir}/zenoh-grpc-client-sdk/zenoh-grpc-python/LICENSE"

rm -f "${work_dir}/zenoh-grpc-client-sdk/zenoh-grpc-python/zenoh_grpc/"*.so

python3 - "${work_dir}/Cargo.toml" <<'PY'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
replacement = """members = [
  "zenoh-grpc-proto",
  "zenoh-grpc-client-sdk/zenoh-grpc-client-rs",
  "zenoh-grpc-client-sdk/zenoh-grpc-python",
]"""
text = re.sub(r'members = \[(.*?)\]', replacement, text, count=1, flags=re.S)
path.write_text(text, encoding="utf-8")
PY

python3 - "${work_dir}/zenoh-grpc-client-sdk/zenoh-grpc-python/pyproject.toml" "${python_version}" <<'PY'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
version = sys.argv[2]
text = path.read_text(encoding="utf-8")
text = re.sub(r'^version = ".*"$', f'version = "{version}"', text, flags=re.M)

if 'readme = "README.md"' not in text:
    text = text.replace('description = "Python bindings for zenoh-plugin-grpc"\n',
                        'description = "Python bindings for zenoh-plugin-grpc"\nreadme = "README.md"\n')
if 'license = { file = "LICENSE" }' not in text:
    text = text.replace('readme = "README.md"\n',
                        'readme = "README.md"\nlicense = { file = "LICENSE" }\n')
path.write_text(text, encoding="utf-8")
PY

(
    cd "${work_dir}/zenoh-grpc-client-sdk/zenoh-grpc-python"
    maturin sdist --out "${out_dir}"
    maturin build --release --out "${out_dir}"
)
