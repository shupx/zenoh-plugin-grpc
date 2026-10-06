# Zenoh 1.10.1 Compatibility Results

Verified on 2026-10-06 on Linux x86_64, Rust/Cargo 1.97.1, Python 3.10.12,
protoc 3.12.4, and GCC 11.4.0.

Local Zenoh source revision: `1211779c3647f5a96713dade452c546a07823580`
(workspace version 1.10.1). The upstream checkout was not modified.

## Changes

- All five direct Zenoh dependencies are pinned to `=1.10.1` from crates.io.
  The lockfile contains no Zenoh 1.7.2 or Git dependencies.
- Workspace and Python package versions are 1.10.1. Rust 1.97.1 matches the
  local Zenoh checkout and is also selected in the build image.
- `.cargo/local-zenoh.toml` permits testing against the sibling source checkout.
  The committed lockfile is restored to registry dependencies after these tests.
- Added native Zenoh/UDS and query error/delete regression coverage. Test sockets
  are unique, and multicast scouting is disabled in the test harness.
- Nightly builds now run the full Rust workspace suite before packaging.

## Results

| Check | Result |
| --- | --- |
| Registry `cargo test --workspace --all-targets --locked` | 18 passed |
| Local-source `cargo --config .cargo/local-zenoh.toml test --workspace --all-targets` | 18 passed |
| Registry workspace tests including doctests | Passed; no doctest cases defined |
| Plugin tests with `--no-default-features --locked` | 12 passed |
| `cargo build --workspace --all-targets` with local source | Passed |
| `cargo build --workspace --release --locked` | Passed |
| Local-source `cargo build -p zenohd --locked` | Passed |
| Local-source debug bridge and dynamic plugin SDK smoke tests | Passed |
| Registry release bridge and dynamic plugin SDK smoke tests | Passed |
| All seven C and seven C++ examples, debug and release | Built and exercised against both server modes |
| Installed Python wheel against both release server modes | Passed, including all C/C++ examples and reconnection |
| `packaging/scripts/build-all.sh` | Five Debian packages, wheel, and sdist generated |
| Debian metadata and extracted executable/library payloads | Version correct; payloads match release artifacts; bridge `--version` succeeds |
| Independent wheel rebuild from the generated sdist | Passed |
| `cargo clippy --workspace --all-targets --locked` | Completed with existing SDK/tonic warnings |
| Edited plugin Rust file formatting and `git diff --check` | Passed |

Process tests cover TCP and UDS, Python callbacks, metadata and delete samples,
session queries, declared queriers, C/C++ pull and callback subscribers/queryables,
and real TCP routing to a native Zenoh Python 1.10.1 client. Server restart tests
retain the same SDK session and publisher/subscriber objects, then require a new
payload to arrive after reconnection and automatic declaration restoration.

## Artifacts And Logs

- Release binaries/libraries: `target/release/`.
- Debian packages, Python wheel/sdist, and build manifest: `dist/linux-amd64/`.
- Local-source server logs: `target/smoke-local-bridge-all/`,
  `target/smoke-local-plugin/`, and the corresponding `*-restart/` directories.
- Release logs: `target/smoke-release-bridge/`, `target/smoke-release-plugin/`.
- Installed-wheel logs: `target/smoke-wheel-bridge/`, `target/smoke-wheel-plugin/`.
- Wheel independently rebuilt from sdist: `target/sdist-wheel/`.

Reproduction commands are in [README.md](README.md).

## Remaining Limits

Validation covers Linux amd64 and Python 3.10. ARM64, Windows, macOS, other
Python versions, and the Docker image build were not executed here.

Workspace-wide `cargo fmt --all -- --check` reports pre-existing formatting
differences in other SDK files. Clippy reports existing warnings, including
large error types, API argument counts, and C FFI safety documentation.

An existing SDK shutdown behavior was observed while developing the restart
harness: `GrpcPublisher::undeclare()` waits for its outgoing queue to empty and
can block if the server is offline with queued writes. The process tests close
their SDK objects while the restarted server is still available. Offline
publisher shutdown was not changed as part of this version migration.
