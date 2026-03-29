import zenoh_grpc


with zenoh_grpc.Session.connect() as session:
    for reply in session.get("demo/query/c", 
                             payload=b"hahaha", 
                             consolidation=zenoh_grpc.ConsolidationMode.NONE, 
                             encoding="text/plain", 
                             timeout_ms=3_000):
        # The querier will receive the replies one by one or all at once depending on the consolidation mode set by the querier (AUTO(default,LATEST), NONE(receive one by one), MONOTONIC(monotonically consolidation), LATEST(only the latest)).
        # the encoding is just a hint for the queryable to know how to parse the payload, it does not affect the actual payload sent to the queryable.
        if reply.ok:
            print("sample:", reply.sample.key_expr, reply.sample.payload, reply.sample.encoding)
        else:
            print("error:", reply.error.payload, reply.error.encoding)
