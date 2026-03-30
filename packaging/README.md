# Packaging

This directory contains scripts and templates used to produce release artifacts for:

- `zenoh-bridge-grpc` Debian package
- `zenoh-plugin-grpc` Debian package
- `zenoh-grpc-client-rs` Debian source-development package
- `zenoh-grpc-c` Debian development package
- `zenoh-grpc-cpp` Debian development package
- `zenoh-grpc-python` wheel and source distribution for PyPI

## Requirements

Native builds are the default and recommended release path. Build on each target platform separately:

- `x86_64 Linux` -> Debian `amd64`
- `aarch64 Linux` -> Debian `arm64`

Required tools on the build host:

- `bash`
- `cargo`
- `dpkg-deb`
- `dpkg-architecture`
- `python3`
- `maturin`

## Versioning

The packaging scripts derive versions from the workspace version plus git state.

Canonical logical version:

```text
1.7.2-dev-20260330.1-g2ca8632
1.7.2-dev-20260330.1-g2ca8632-modified
```

Debian version:

```text
1.7.2~dev20260330.1+g2ca8632-1
1.7.2~dev20260330.1+g2ca8632.modified-1
```

Python public version:

```text
1.7.2.dev2026033001
```

The Python version intentionally omits the git hash and dirty suffix so it remains a valid public PyPI version under PEP 440.

## Usage

Build everything:

```bash
packaging/scripts/build-all.sh --date 20260330 --seq 1
```

Build selected artifacts:

```bash
packaging/scripts/build-all.sh --date 20260330 --seq 1 --only bridge,plugin,python
```

Write outputs to a custom directory:

```bash
packaging/scripts/build-all.sh --date 20260330 --seq 1 --out-dir /tmp/releases
```

Optional environment variables:

- `BUILD_DATE`: defaults to current UTC date in `YYYYMMDD`
- `BUILD_SEQ`: required in CI; defaults to `1` for local builds
- `RUST_TARGET`: optional cargo target triple for non-default native output layout

## Outputs

Artifacts are written under:

```text
dist/linux-amd64/
dist/linux-arm64/
```

Each run also generates:

- `manifest.json`: machine-readable artifact metadata
- `BUILD_INFO.txt`: human-readable release summary

## Package Notes

- `zenoh-grpc-client-rs` is packaged as a source-development package on purpose. It does not ship a public precompiled Rust ABI.
- `zenoh-grpc-cpp` ships headers plus CMake package files and depends on `zenoh-grpc-c`.
- Package docs are staged into `/usr/share/doc/<package>/` and include the relevant README, license, and a generated Debian changelog.
