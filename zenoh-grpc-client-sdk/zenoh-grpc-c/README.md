# zenoh-grpc-c

C ABI wrapper for `zenoh-plugin-grpc`.

## Build

```bash
cargo build -p zenoh-grpc-c --release
```

Artifacts are produced under:

```bash
target/release/
```

## Start A Server First

```bash
cargo run -p zenoh-bridge-grpc -- --grpc-host 127.0.0.1 --grpc-port 7335
```

## Current Scope

The current C layer exposes the session open/close entrypoints first. More Zenoh-like handles can be added on top of the shared Rust client core next.

## Examples

See:

- `examples/z_put.c`
- `examples/z_sub.c`
- `examples/z_queryable.c`
- `examples/z_querier.c`
