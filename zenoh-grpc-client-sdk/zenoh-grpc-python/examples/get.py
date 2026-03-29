import zenoh_grpc


with zenoh_grpc.Session.connect() as session:
    for reply in session.get("demo/query/c", payload=b"hahaha", encoding="text/plain", timeout_ms=3_000):
        if reply.ok:
            print("sample:", reply.sample.key_expr, reply.sample.payload, reply.sample.encoding)
        else:
            print("error:", reply.error.payload, reply.error.encoding)
