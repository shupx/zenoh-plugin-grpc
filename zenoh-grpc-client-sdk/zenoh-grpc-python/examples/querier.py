import zenoh_grpc


with zenoh_grpc.Session.connect() as session:
    querier = session.declare_querier("demo/query/**", consolidation=zenoh_grpc.ConsolidationMode.NONE, timeout_ms=3_000)

    replies = querier.get(payload=b"query from python", encoding="text/plain") # unblocking, returns a ReplyStream

    for reply in replies:
        if reply.ok:
            print("sample:", reply.sample.key_expr, reply.sample.payload, reply.sample.encoding)
        else:
            print("error:", reply.error.payload, reply.error.encoding)
