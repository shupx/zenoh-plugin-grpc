import zenoh_grpc


with zenoh_grpc.Session.connect("tcp://127.0.0.1:7335") as session:
    print(session.get("demo/query/**", timeout_ms=3_000))
