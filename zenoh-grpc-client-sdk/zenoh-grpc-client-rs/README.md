# zenoh-grpc-client-rs

Rust client core for `zenoh-plugin-grpc`.

The public API uses request structs instead of simplified helper overloads, so callers can pass the gRPC-visible options directly.

`put/delete/reply*` now use local enqueue semantics: a successful call means the request entered a local bounded queue, not that the remote gRPC server has already processed it. When the local send queue is full, the oldest queued item is dropped.

Streamed receives (`subscriber`, `queryable`, `session.get`, `querier.get`) also use local bounded queues with drop-oldest behavior, so slow consumers do not backpressure the gRPC stream reader. Use `dropped_count()` on receivers and `send_dropped_count()` on send-capable objects to observe overflow.

## Build

```bash
cargo build -p zenoh-grpc-client-rs
```

## Connect

TCP:

```rust
use zenoh_grpc_client_rs::{ConnectAddr, GrpcSession};

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let session = GrpcSession::connect(ConnectAddr::Tcp("127.0.0.1:7335".into())).await?;
# Ok(())
# }
```

UDS:

```rust
use std::path::PathBuf;
use zenoh_grpc_client_rs::{ConnectAddr, GrpcSession};

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let session = GrpcSession::connect(ConnectAddr::Unix(PathBuf::from("/tmp/zenoh-grpc.sock"))).await?;
# Ok(())
# }
```

## Request-Style API

```rust
use zenoh_grpc_client_rs::{
    ConnectAddr, DeclarePublisherArgs, GrpcSession, PublisherPutArgs, SessionGetArgs,
};

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let session = GrpcSession::connect(ConnectAddr::Tcp("127.0.0.1:7335".into())).await?;

let publisher = session
    .declare_publisher(DeclarePublisherArgs {
        key_expr: "demo/example/value".into(),
        encoding: "text/plain".into(),
        ..Default::default()
    })
    .await?;

publisher
    .put(PublisherPutArgs {
        payload: b"hello".to_vec(),
        timestamp: "0".into(),
        ..Default::default()
    })
    .await?;

println!("publisher send drops: {}", publisher.send_dropped_count());

let _replies = session
    .get(SessionGetArgs {
        selector: "demo/example/value".into(),
        ..Default::default()
    })
    .await?;
# Ok(())
# }
```

## Examples

Run from the workspace root:

```bash
cargo run -p zenoh-grpc-client-rs --example info
cargo run -p zenoh-grpc-client-rs --example pub_sub
cargo run -p zenoh-grpc-client-rs --example queryable
```
