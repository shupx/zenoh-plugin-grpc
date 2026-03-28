# zenoh-plugin-grpc

Rust gRPC plugin, bridge, and SDKs for Zenoh `1.7.2`.

## Workspace

- `zenoh-plugin-grpc`: plugin loaded by `zenohd`
- `zenoh-bridge-grpc`: standalone executable with the plugin linked in
- `zenoh-grpc-proto`: gRPC proto and generated Rust types
- `zenoh-grpc-client-sdk/zenoh-grpc-client-rs`: Rust client core
- `zenoh-grpc-client-sdk/zenoh-grpc-python`: Python bindings
- `zenoh-grpc-client-sdk/zenoh-grpc-c`: C wrapper
- `zenoh-grpc-client-sdk/zenoh-grpc-cpp`: C++ wrapper

## Build

```bash
cargo check --workspace
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

- [zenoh-plugin-grpc/README.md](/home/spx/spx_ws/zenoh_develop/zenoh-plugin-grpc/zenoh-plugin-grpc/README.md)
- [zenoh-bridge-grpc/README.md](/home/spx/spx_ws/zenoh_develop/zenoh-plugin-grpc/zenoh-bridge-grpc/README.md)
