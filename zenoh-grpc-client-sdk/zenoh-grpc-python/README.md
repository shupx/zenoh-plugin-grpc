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
cargo run -p zenoh-bridge-grpc
```

or `zenohd` with the plugin loaded.

## Minimal Example

```python
import zenoh_grpc

with zenoh_grpc.Session.connect() as session:
    pub = session.declare_publisher("demo/example", encoding="text/plain")
    pub.put(b"hello", encoding="text/plain")
```

`Session.connect()` defaults to `unix:///tmp/zenoh-grpc.sock`, so the shortest form is:

```python
with zenoh_grpc.Session.connect() as session:
    ...
```

## Python-Style Optional Parameters

The binding keeps Python-style direct function calls and exposes extra gRPC options as optional keyword arguments. If you omit them, the plugin now falls back to Zenoh's native defaults instead of hard-coded wrapper defaults.

`put/delete/reply*` calls are enqueue-style: success means the request entered a local bounded queue. Slow receivers are also isolated behind local bounded queues, and when a queue is full the oldest item is dropped. `Subscriber.dropped_count()`, `Queryable.dropped_count()`, `ReplyStream.dropped_count()`, `Publisher.send_dropped_count()`, and `Queryable.send_dropped_count()` expose those counters.

`declare_subscriber(key_expr, callback=None, ...)` and `declare_queryable(key_expr, callback=None, ...)` support inline callbacks. When a callback is provided, the binding runs it on a dedicated OS thread for that object and acquires the GIL for each event. Callback mode is exclusive with manual `recv()/try_recv()`. Queryables now deliver `Query` objects, and `Session.get()` / `Querier.get()` return iterable `ReplyStream` objects.

```python
import zenoh_grpc

with zenoh_grpc.Session.connect() as session:
    pub = session.declare_publisher(
        "demo/example",
        encoding="text/plain",
        express=True,
    )
    pub.put(b"hello", encoding="text/plain")

    for reply in session.get("demo/example", timeout_ms=3000):
        if reply.ok:
            print(reply.sample.key_expr, reply.sample.payload)
```

Supported endpoint formats:

- `unix:///tmp/zenoh-grpc.sock`
- `tcp://127.0.0.1:7335`

## Examples

See:

- `examples/pub.py`
- `examples/sub.py`
- `examples/sub_callback.py`
- `examples/queryable.py`
- `examples/queryable_callback.py`
- `examples/querier.py`
- `examples/get.py`
