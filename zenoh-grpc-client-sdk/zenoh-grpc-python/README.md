# zenoh-grpc-python

Python bindings for `zenoh-plugin-grpc`.

## Install

From this directory:

```bash
maturin develop
```

Or build a wheel:

```bash
maturin build
```

## Start A Server First

Use either:

```bash
cargo run -p zenoh-bridge-grpc -- --grpc-host 127.0.0.1 --grpc-port 7335
```

or `zenohd` with the plugin loaded.

## Minimal Example

```python
import zenoh_grpc

with zenoh_grpc.Session.connect("tcp://127.0.0.1:7335") as session:
    pub = session.declare_publisher("demo/example", encoding="text/plain")
    pub.put(b"hello", encoding="text/plain")
```

`Session.connect()` defaults to `tcp://127.0.0.1:7335`, so the shortest form is:

```python
with zenoh_grpc.Session.connect() as session:
    ...
```

## Python-Style Optional Parameters

The binding keeps Python-style direct function calls and exposes extra gRPC options as optional keyword arguments. If you omit them, the plugin now falls back to Zenoh's native defaults instead of hard-coded wrapper defaults.

`put/delete/reply*` calls are enqueue-style: success means the request entered a local bounded queue. Slow receivers are also isolated behind local bounded queues, and when a queue is full the oldest item is dropped. `Subscriber.dropped_count()`, `Queryable.dropped_count()`, `Publisher.send_dropped_count()`, and `Queryable.send_dropped_count()` expose those counters.

```python
import zenoh_grpc

with zenoh_grpc.Session.connect("tcp://127.0.0.1:7335") as session:
    pub = session.declare_publisher(
        "demo/example",
        encoding="text/plain",
        express=True,
    )
    pub.put(b"hello", encoding="text/plain")

    replies = session.get("demo/example", timeout_ms=3000)
```

Supported endpoint formats:

- `tcp://127.0.0.1:7335`
- `unix:///tmp/zenoh-grpc.sock`

## Examples

See:

- `examples/pub.py`
- `examples/sub.py`
- `examples/sub_callback.py`
- `examples/queryable.py`
- `examples/queryable_callback.py`
- `examples/get.py`
