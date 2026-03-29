import zenoh_grpc


with zenoh_grpc.Session.connect() as session:
    querier = session.declare_querier("demo/query/**", timeout_ms=3_000)
    replies = querier.get(payload=b"query from python", encoding="text/plain")
    print("received replies:", replies)
