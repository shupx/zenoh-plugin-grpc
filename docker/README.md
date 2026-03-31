# CI build image

This image is intended to provide the native build environment used by CI for:

- `packaging/scripts/build-all.sh`
- Debian package generation for `amd64` and `arm64`
- Python package builds through `maturin`

## Build

Use `docker buildx` to publish both architectures:

```bash
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t <your-registry>/zenoh-plugin-grpc-build:latest \
  --push \
  .
```

## Use in CI

The image pre-installs:

- Ubuntu 20.04
- Rust 1.85.0 from `rustup`
- `python3`, `pip`, `maturin`
- `git`
- `protobuf-compiler`
- `dpkg-dev`

It also pre-runs:

```bash
cargo build --workspace --release --locked
```

This warm-up build is done against minimal placeholder sources for the local workspace crates,
so the image caches registry/git dependencies and most third-party compilation work without
baking the full repository sources into the image itself.

To maximize reuse of the baked dependency cache in CI, keep `CARGO_TARGET_DIR=/opt/zenoh-target`
inside the container when running `cargo` or `packaging/scripts/build-all.sh`.

Run CI against the checked-out repository mounted into the container so packaging scripts
can still read the live `.git` metadata used for version generation.
