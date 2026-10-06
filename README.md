# zenoh-plugin-grpc


[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/shupx/zenoh-plugin-grpc)

This plugin starts a gRPC server inside zenohd and exposes Zenoh operations over gRPC so that external applications (python/c++) can interact with Zenoh via gRPC calls. 

This is useful for one zenoh peer/router acting as a communication bridge and multiple external (local) applications connecting to it to send and receive messages. 

Supported zenoh versions:

- 1.10.1: current branch. Rust gRPC plugin, bridge, and SDKs for Zenoh `1.10.1` and Rust `1.97.1`.

- 1.7.2: please switch to `1.7.2` branch. Rust gRPC plugin, bridge, and SDKs for Zenoh `1.7.2` and rust `1.85.0`.

## Workspace

- `zenoh-plugin-grpc`: plugin loaded by `zenohd`
- `zenoh-bridge-grpc`: standalone executable with the plugin linked in
- `zenoh-grpc-proto`: gRPC proto and generated Rust types, used by gRPC server (`zenoh-plugin-grpc`) and clients (`zenoh-grpc-client-rs`).
- `zenoh-grpc-client-sdk/zenoh-grpc-client-rs`: Rust client core
- `zenoh-grpc-client-sdk/zenoh-grpc-python`: Python bindings
- `zenoh-grpc-client-sdk/zenoh-grpc-c`: C wrapper
- `zenoh-grpc-client-sdk/zenoh-grpc-cpp`: C++ wrapper

## Build

```bash
cargo check --workspace
cargo build --release
```

Zenoh dependencies are pinned to `=1.10.1`. Build `zenohd` and the dynamic
plugin with the same Rust toolchain (`1.97.1`) for plugin ABI compatibility.

## Tests

```bash
cargo test --workspace --all-targets --locked
cargo test --workspace --doc --locked
```

To test against a local Zenoh 1.10.1 checkout in `../zenoh`, opt in to the
source overrides. Cargo updates the lockfile for these overrides, so preserve
the registry lockfile before running and restore it afterward:

```bash
mkdir -p target
cp Cargo.lock target/registry-Cargo.lock
cargo --config .cargo/local-zenoh.toml test --workspace --all-targets
cp target/registry-Cargo.lock Cargo.lock
```

See [tests/README.md](tests/README.md) for process-level SDK and dynamic plugin
tests, including native Zenoh transport and server restart checks.

## Quick Start

Start the standalone bridge:

```bash
cargo run -p zenoh-bridge-grpc
```

Then connect from a client SDK to:

```text
unix:///tmp/zenoh-grpc.sock
```

## Plugin Mode

If you want to run inside `zenohd`, see:

- [zenoh-plugin-grpc/README.md](zenoh-plugin-grpc/README.md)
- [zenoh-bridge-grpc/README.md](zenoh-bridge-grpc/README.md)

## gRPC Client SDK

- [zenoh-grpc-client-sdk/README.md](zenoh-grpc-client-sdk/README.md)
