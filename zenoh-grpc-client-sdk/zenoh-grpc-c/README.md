# zenoh-grpc-c

`zenoh-grpc-c` is the C ABI wrapper for `zenoh-plugin-grpc`.

It now exposes:

- session `connect/info/put/delete/get`
- publisher / subscriber / queryable / querier handles
- pull mode and callback mode for subscriber/queryable
- reply streams for `Session.get()` and `Querier.get()`
- Zenoh-like option structs with `*_default(...)` helpers

## Prerequisites

Start a gRPC server first. Either:

```bash
cargo run -p zenoh-bridge-grpc
```

or run `zenohd` with `zenoh-plugin-grpc` loaded.

The examples default to:

```text
unix:///tmp/zenoh-grpc.sock
```

You can override the endpoint by passing it as the first CLI argument.

## Build The C Library

From the workspace root:

```bash
cargo build -p zenoh-grpc-c --release
```

Artifacts are generated in:

```text
target/release/libzenohgrpc.so
target/release/libzenohgrpc.a
```

Header:

```text
zenoh-grpc-client-sdk/zenoh-grpc-c/include/zenoh_grpc.h
```

## Build The Examples With CMake

From `zenoh-plugin-grpc/`:

```bash
cmake -S zenoh-grpc-client-sdk/zenoh-grpc-c -B build/zenoh-grpc-c
cmake --build build/zenoh-grpc-c
```

This builds:

- `z_pub`
- `z_sub`
- `z_sub_callback`
- `z_queryable`
- `z_queryable_callback`
- `z_querier`
- `z_get`

## Manual Compile

Example:

```bash
cc zenoh-grpc-client-sdk/zenoh-grpc-c/examples/z_pub.c \
  -I zenoh-grpc-client-sdk/zenoh-grpc-c/include \
  -I zenoh-grpc-client-sdk/zenoh-grpc-c/examples \
  -L target/release \
  -lzenohgrpc \
  -Wl,-rpath,$PWD/target/release \
  -o /tmp/zgrpc_c_pub
```

## Install

`zenoh-grpc-c` itself is built by Cargo. A simple local install layout is:

```bash
install -Dm644 zenoh-grpc-client-sdk/zenoh-grpc-c/include/zenoh_grpc.h /usr/local/include/zenoh_grpc.h
install -Dm755 target/release/libzenohgrpc.so /usr/local/lib/libzenohgrpc.so
install -Dm644 target/release/libzenohgrpc.a /usr/local/lib/libzenohgrpc.a
```

## Run Examples

With the default Unix socket:

```bash
./build/zenoh-grpc-c/z_queryable
./build/zenoh-grpc-c/z_querier
```

With an explicit TCP endpoint:

```bash
./build/zenoh-grpc-c/z_pub tcp://127.0.0.1:7335
./build/zenoh-grpc-c/z_sub tcp://127.0.0.1:7335
```

## Notes

- Callback mode runs on background threads owned by the wrapper.
- Callback invocations for the same subscriber/queryable are serialized.
- Callback event/query pointers are borrowed for the duration of the callback only. Copy data if you need to keep it.
- Query replies should be finalized with `zgrpc_queryable_finish(...)` when you are done replying.
