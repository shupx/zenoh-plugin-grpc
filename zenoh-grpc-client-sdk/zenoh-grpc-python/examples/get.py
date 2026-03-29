import zenoh_grpc


with zenoh_grpc.Session.connect() as session:
    print(session.get("demo/query/c", payload=b'hahaha', encoding="text/plain", timeout_ms=3_000))
