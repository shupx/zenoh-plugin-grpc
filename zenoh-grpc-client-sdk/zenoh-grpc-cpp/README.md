# zenoh-grpc-cpp

C++ thin wrapper for `zenoh-grpc-c`.

## Build

This layer currently provides headers and a small CMake target:

```bash
cmake -S zenoh-grpc-client-sdk/zenoh-grpc-cpp -B build
cmake --build build
```

## Start A Server First

```bash
cargo run -p zenoh-bridge-grpc -- --grpc-host 127.0.0.1 --grpc-port 7335
```

## Minimal Example

```cpp
#include <zenoh_grpc/session.hpp>

int main() {
    auto session = zenoh_grpc::Session::open_tcp("127.0.0.1:7335");
    return session.valid() ? 0 : 1;
}
```

## Examples

See:

- `examples/pub.cpp`
- `examples/sub.cpp`
- `examples/queryable.cpp`
- `examples/querier.cpp`
