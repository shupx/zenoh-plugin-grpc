# CI build image

This image is intended to provide the native build environment used by CI for:

- `packaging/scripts/build-all.sh`
- Debian package generation for `amd64` and `arm64`
- Python package builds through `maturin`

## Build

From the repository root, use `docker buildx` to publish both architectures
with the version tag referenced by the nightly workflow:

```bash
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t shupeixuan/zenoh-plugin-grpc-build:zenoh-1.10.1-rust-1.97.1 \
  --push \
  .
```

## Run

```bash
# create container
docker run -dit --name zenoh_grpc_build -v /home/${USER}:/home/${USER} shupeixuan/zenoh-plugin-grpc-build:zenoh-1.10.1-rust-1.97.1 tail -f /dev/null
# enter container
docker exec -it zenoh_grpc_build /bin/bash
```

## Use in CI

The image pre-installs:

- Ubuntu 20.04
- Rust 1.97.1 from `rustup`
- `python3`, `pip`, `maturin`
- `git`
- `protobuf-compiler`
- `dpkg-dev`

It also pre-runs:

```bash
cargo fetch --locked
```

This warm-up step is done against minimal placeholder sources for the local workspace crates,
so the image caches registry and git dependencies without baking the full repository sources
or release build artifacts into the image itself.

The main reusable cache lives under `/root/.cargo`, including crates.io downloads and git
dependencies, including the exact Zenoh 1.10.1 crates.

Run CI against the checked-out repository mounted into the container so packaging scripts
can still read the live `.git` metadata used for version generation.
