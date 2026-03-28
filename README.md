# zenoh-plugin-grpc

Rust gRPC plugin, bridge, and SDKs for Zenoh `1.7.2` and rust `1.85.0` now.

This plugin starts a gRPC server inside zenohd and exposes Zenoh operations over gRPC so that external applications (python/c++) can interact with Zenoh via gRPC calls. 

This is useful for one zenoh peer/router acting as a communication bridge and multiple external (local) applications connecting to it to send and receive messages. 

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
