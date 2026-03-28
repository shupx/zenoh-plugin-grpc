# zenoh-bridge-grpc

Standalone executable with the gRPC plugin already linked in.

## Build

```bash
cargo build -p zenoh-bridge-grpc
```

## Run

Minimal start:

```bash
cargo run -p zenoh-bridge-grpc
```

With an explicit UDS listener path:

```bash
cargo run -p zenoh-bridge-grpc -- --grpc-uds /tmp/zenoh-grpc.sock
```

With normal Zenoh arguments:

```bash
cargo run -p zenoh-bridge-grpc -- \
  --mode peer \
  --listen tcp/127.0.0.1:7447 \
  --connect tcp/127.0.0.1:7448 \
  --grpc-host 127.0.0.1 \
  --grpc-port 7335
```
