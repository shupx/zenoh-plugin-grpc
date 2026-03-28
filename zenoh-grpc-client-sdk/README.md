# zenoh-grpc-client-sdk

Client SDKs built on top of the shared Rust gRPC client core.

## Layout

- `zenoh-grpc-client-rs`: Rust client core
- `zenoh-grpc-python`: Python bindings
- `zenoh-grpc-c`: C ABI wrapper
- `zenoh-grpc-cpp`: C++ thin wrapper

## Build Everything

From the workspace root:

```bash
cargo check --workspace
```

## Server Requirement

Before using any SDK, start either:

- `zenohd` with `zenoh-plugin-grpc` loaded, or
- `zenoh-bridge-grpc`

By default the SDKs connect to the gRPC server address you pass in, typically:

```text
127.0.0.1:7335
```
