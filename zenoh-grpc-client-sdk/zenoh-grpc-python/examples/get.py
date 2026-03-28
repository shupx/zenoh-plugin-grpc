import zenoh_grpc


with zenoh_grpc.Session.connect() as session:
    print(session.get("demo/query/**", timeout_ms=3_000))
