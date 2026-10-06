# Compatibility Tests

The Rust workspace suite covers configuration, client queues and offline
operations, TCP pub/sub, queryables with session and declared-querier queries,
query completion/drop, lease cleanup, UDS interoperability with native Zenoh,
metadata, deletes, and error/delete query replies.

`sdk_smoke.py` launches a real server with isolated ports and a temporary UDS.
It checks the Python SDK over TCP and UDS, communication with the native Zenoh
Python 1.10.1 client over TCP, metadata, deletes, queryables, queriers, callbacks,
and publisher/subscriber restoration after a server restart. When example
directories are supplied, it also runs all seven C and all seven C++ examples.
Every launched process is terminated at the end of its check. Logs remain in
the directory supplied with `--logs`.

## Setup

From the workspace root, with Rust 1.97.1, a C/C++ compiler, CMake, protoc, and
Python 3.8 or newer installed:

```bash
cargo build --workspace --release --locked
python3 -m venv target/test-venv
target/test-venv/bin/pip install maturin eclipse-zenoh==1.10.1
PATH="$PWD/target/test-venv/bin:$PATH" maturin build --release --locked \
  --manifest-path zenoh-grpc-client-sdk/zenoh-grpc-python/Cargo.toml \
  --out target/wheels
target/test-venv/bin/pip install target/wheels/zenoh_grpc-1.10.1-*.whl

cmake -S zenoh-grpc-client-sdk/zenoh-grpc-c -B target/c-examples \
  -DZENOH_GRPC_C_LIBDIR="$PWD/target/release"
cmake --build target/c-examples
cmake -S zenoh-grpc-client-sdk/zenoh-grpc-cpp -B target/cpp-examples \
  -DZENOH_GRPC_C_LIBDIR="$PWD/target/release"
cmake --build target/cpp-examples
```

## Standalone Bridge

```bash
timeout 120 target/test-venv/bin/python tests/sdk_smoke.py \
  --server target/release/zenoh-bridge-grpc \
  --c-examples target/c-examples --cpp-examples target/cpp-examples \
  --logs target/smoke-bridge
```

## Dynamic Plugin

Build the local Zenoh 1.10.1 router with the same toolchain:

```bash
cargo build --manifest-path ../zenoh/Cargo.toml -p zenohd --locked
timeout 120 target/test-venv/bin/python tests/sdk_smoke.py \
  --server ../zenoh/target/debug/zenohd \
  --plugin target/release/libzenoh_plugin_grpc.so \
  --c-examples target/c-examples --cpp-examples target/cpp-examples \
  --logs target/smoke-plugin
```

The router configuration marks the plugin as required, so failure to load its
dynamic library fails the test. These commands target Linux; library filenames
and UDS availability differ on other platforms.
