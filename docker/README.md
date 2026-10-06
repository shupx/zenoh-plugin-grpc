# CI build image

This image is intended to provide the native build environment used by CI for:

- `packaging/scripts/build-all.sh`
- Debian package generation for `amd64` and `arm64`
- Python package builds through `maturin`

## Build With GitHub Actions

The manually triggered [Build Docker image](../.github/workflows/docker-build.yml)
workflow builds on native GitHub-hosted runners:

- `ubuntu-24.04` for `linux/amd64`.
- `ubuntu-24.04-arm` for `linux/arm64`.

Both jobs build the Ubuntu 20.04 image defined by the Dockerfile and push their
images by digest. Once both succeed, the publish job combines them into one
multi-architecture version tag. No QEMU emulation is used.

Configure these repository secrets under **Settings > Secrets and variables >
Actions**:

- `DOCKERHUB_USERNAME`: the Docker Hub account with push access to
  `shupeixuan/zenoh-plugin-grpc-build`.
- `DOCKERHUB_TOKEN`: a Docker Hub access token with write permission for that
  repository.

Commit the workflow to the default branch. In **Actions > Build Docker image >
Run workflow**, select the branch to build and start the workflow. The image tag
is derived from the exact Zenoh dependency in `Cargo.toml` and the toolchain in
`rust-toolchain.toml`, currently `zenoh-1.10.1-rust-1.97.1`. It does not publish
`latest`; rerunning the same version replaces that version tag.

After the workflow succeeds, inspect the published platforms:

```bash
docker buildx imagetools inspect \
  shupeixuan/zenoh-plugin-grpc-build:zenoh-1.10.1-rust-1.97.1
```

Run the nightly workflow after publishing this image. When upgrading Zenoh or
Rust again, also update both image references in `nightly-build.yml` to match
the newly generated tag. Keep the Dockerfile's installed Rust version aligned
with `rust-toolchain.toml`.

The ARM64 job requires repository access to GitHub's `ubuntu-24.04-arm` runner.

## Build Locally

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
