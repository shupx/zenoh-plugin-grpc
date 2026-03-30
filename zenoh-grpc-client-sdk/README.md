# zenoh-grpc-client-sdk

Client SDKs built on top of the shared Rust gRPC client core.

## Layout

- `zenoh-grpc-client-rs`: shared Rust client core
- `zenoh-grpc-python`: Python bindings
- `zenoh-grpc-c`: C ABI wrapper
- `zenoh-grpc-cpp`: header-only C++ wrapper on top of `zenoh-grpc-c`

## Server Requirement

Before using any SDK, start either:

- `zenohd` with `zenoh-plugin-grpc` loaded, or
- `zenoh-bridge-grpc`

Default endpoint:

```text
unix:///tmp/zenoh-grpc.sock
```

TCP endpoints are also supported, for example:

```text
tcp://127.0.0.1:7335
```

## Build Matrix

From the workspace root:

```bash
cargo check -p zenoh-grpc-client-rs
cargo check -p zenoh-grpc-python
cargo check -p zenoh-grpc-c
```

Build the C library for C/C++ consumption:

```bash
cargo build -p zenoh-grpc-c --release
```

Then:

```bash
cmake -S zenoh-grpc-client-sdk/zenoh-grpc-c -B build/zenoh-grpc-c
cmake --build build/zenoh-grpc-c

cmake -S zenoh-grpc-client-sdk/zenoh-grpc-cpp -B build/zenoh-grpc-cpp
cmake --build build/zenoh-grpc-cpp
```

## Examples

Python examples:

- `examples/pub.py`
- `examples/sub.py`
- `examples/sub_callback.py`
- `examples/queryable.py`
- `examples/queryable_callback.py`
- `examples/querier.py`
- `examples/get.py`

C examples:

- `examples/z_pub.c`
- `examples/z_sub.c`
- `examples/z_sub_callback.c`
- `examples/z_queryable.c`
- `examples/z_queryable_callback.c`
- `examples/z_querier.c`
- `examples/z_get.c`

C++ examples:

- `examples/pub.cpp`
- `examples/sub.cpp`
- `examples/sub_callback.cpp`
- `examples/queryable.cpp`
- `examples/queryable_callback.cpp`
- `examples/querier.cpp`
- `examples/get.cpp`
