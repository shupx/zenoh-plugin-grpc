# zenoh-grpc-cpp

`zenoh-grpc-cpp` is a header-only C++ RAII wrapper built on top of `zenoh-grpc-c`.

It exposes:

- `Session`, `Publisher`, `Subscriber`, `Queryable`, `Querier`, `ReplyStream`, `Query`
- Zenoh-like options structs
- pull mode and callback mode
- automatic undeclare/close on destruction

## Prerequisites

Build the C library first:

```bash
cargo build -p zenoh-grpc-c --release
```

Start a server first:

```bash
cargo run -p zenoh-bridge-grpc
```

Examples default to:

```text
unix:///tmp/zenoh-grpc.sock
```

## Build With CMake

From `zenoh-plugin-grpc/`:

```bash
cmake -S zenoh-grpc-client-sdk/zenoh-grpc-cpp -B build/zenoh-grpc-cpp
cmake --build build/zenoh-grpc-cpp
```

By default the CMake project looks for `libzenohgrpc` in:

```text
target/release
```

If needed, override it:

```bash
cmake -S zenoh-grpc-client-sdk/zenoh-grpc-cpp -B build/zenoh-grpc-cpp \
  -DZENOH_GRPC_C_LIBDIR=$PWD/target/release \
  -DZENOH_GRPC_C_INCLUDE_DIR=$PWD/zenoh-grpc-client-sdk/zenoh-grpc-c/include
```

## Install

The CMake project installs:

- C++ headers
- `zenoh_grpc.h`
- exported `zenohgrpcxx` interface target

Example:

```bash
cmake --install build/zenoh-grpc-cpp --prefix /tmp/zenoh-grpc-cpp-install
```

`zenohgrpcxx` is header-only; consumers still need to link `libzenohgrpc`.

## Manual Compile

Example:

```bash
c++ -std=c++17 zenoh-grpc-client-sdk/zenoh-grpc-cpp/examples/pub.cpp \
  -I zenoh-grpc-client-sdk/zenoh-grpc-cpp/include \
  -I zenoh-grpc-client-sdk/zenoh-grpc-cpp/examples \
  -I zenoh-grpc-client-sdk/zenoh-grpc-c/include \
  -L target/release \
  -lzenohgrpc \
  -Wl,-rpath,$PWD/target/release \
  -o /tmp/zgrpc_cpp_pub
```

## Examples

The CMake project builds:

- `pub`
- `sub`
- `sub_callback`
- `queryable`
- `queryable_callback`
- `querier`
- `get`

Run them with either the default Unix socket or an explicit endpoint:

```bash
./build/zenoh-grpc-cpp/queryable
./build/zenoh-grpc-cpp/querier
./build/zenoh-grpc-cpp/pub tcp://127.0.0.1:7335
```

## Notes

- Callback-mode `Query` objects auto-finish on destruction, but calling `query.finish()` explicitly is still recommended for clarity.
- `ReplyStream` and pull-mode handles expose `try_recv()`, `dropped_count()` and `is_closed()`.
